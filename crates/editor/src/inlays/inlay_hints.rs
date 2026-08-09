use std::{
    collections::hash_map,
    ops::{ControlFlow, Range},
    time::Duration,
};

use clock::Global;
use collections::{HashMap, HashSet};
use futures::future::join_all;
use gpui::{App, Entity, Pixels, Task};
use itertools::Itertools;
use language::{
    BufferRow,
    language_settings::{InlayHintKind, InlayHintSettings},
};
use lsp::LanguageServerId;
use multi_buffer::{Anchor, MultiBufferSnapshot};
use project::{
    HoverBlock, HoverBlockKind, InlayHintLabel, InlayHintLabelPartTooltip, InlayHintTooltip,
    InvalidationStrategy, ResolveState,
    lsp_store::{CacheInlayHints, ResolvedHint},
};
use text::{Bias, BufferId};
use ui::{Context, Window};
use util::debug_panic;

use super::{Inlay, InlayId};
use crate::{
    Editor, EditorSnapshot, PointForPosition, ToggleInlayHints, ToggleInlineValues, debounce_value,
    display_map::{DisplayMap, InlayOffset},
    hover_links::{InlayHighlight, TriggerPoint, show_link_definition},
    hover_popover::{self, InlayHover},
    inlays::InlaySplice,
};

pub fn inlay_hint_settings(
    location: Anchor,
    snapshot: &MultiBufferSnapshot,
    cx: &mut Context<Editor>,
) -> InlayHintSettings {
    snapshot.language_settings_at(location, cx).inlay_hints
}

#[derive(Debug)]
pub struct LspInlayHintData {
    enabled: bool,
    modifiers_override: bool,
    enabled_in_settings: bool,
    allowed_hint_kinds: HashSet<Option<InlayHintKind>>,
    invalidate_debounce: Option<Duration>,
    append_debounce: Option<Duration>,
    hint_refresh_tasks: HashMap<BufferId, Vec<Task<()>>>,
    hint_chunk_fetching: HashMap<BufferId, (Global, HashSet<Range<BufferRow>>)>,
    invalidate_hints_for_buffers: HashSet<BufferId>,
    pub added_hints: HashMap<InlayId, Option<InlayHintKind>>,
}

impl LspInlayHintData {
    pub fn new(settings: InlayHintSettings) -> Self {
        Self {
            modifiers_override: false,
            enabled: settings.enabled,
            enabled_in_settings: settings.enabled,
            hint_refresh_tasks: HashMap::default(),
            added_hints: HashMap::default(),
            hint_chunk_fetching: HashMap::default(),
            invalidate_hints_for_buffers: HashSet::default(),
            invalidate_debounce: debounce_value(settings.edit_debounce_ms),
            append_debounce: debounce_value(settings.scroll_debounce_ms),
            allowed_hint_kinds: settings.enabled_inlay_hint_kinds(),
        }
    }

    pub fn modifiers_override(&mut self, new_override: bool) -> Option<bool> {
        if self.modifiers_override == new_override {
            return None;
        }
        self.modifiers_override = new_override;
        if (self.enabled && self.modifiers_override) || (!self.enabled && !self.modifiers_override)
        {
            self.clear();
            Some(false)
        } else {
            Some(true)
        }
    }

    pub fn toggle(&mut self, enabled: bool) -> bool {
        if self.enabled == enabled {
            return false;
        }
        self.enabled = enabled;
        self.modifiers_override = false;
        if !enabled {
            self.clear();
        }
        true
    }

    pub fn clear(&mut self) {
        self.hint_refresh_tasks.clear();
        self.hint_chunk_fetching.clear();
        self.added_hints.clear();
    }

    /// Like `clear`, but only wipes tracking state for the given buffer IDs.
    /// Hints belonging to other buffers are left intact so they are neither
    /// re-fetched nor duplicated on the next `NewLinesShown`.
    pub fn clear_for_buffers(
        &mut self,
        buffer_ids: &HashSet<BufferId>,
        current_hints: impl IntoIterator<Item = Inlay>,
        snapshot: &MultiBufferSnapshot,
    ) {
        for buffer_id in buffer_ids {
            self.hint_refresh_tasks.remove(buffer_id);
            self.hint_chunk_fetching.remove(buffer_id);
        }
        for hint in current_hints {
            if let Some((text_anchor, _)) = snapshot.anchor_to_buffer_anchor(hint.position) {
                if buffer_ids.contains(&text_anchor.buffer_id) {
                    self.added_hints.remove(&hint.id);
                }
            }
        }
    }

    /// Checks inlay hint settings for enabled hint kinds and general enabled state.
    /// Generates corresponding inlay_map splice updates on settings changes.
    /// Does not update inlay hint cache state on disabling or inlay hint kinds change: only reenabling forces new LSP queries.
    fn update_settings(
        &mut self,
        new_hint_settings: InlayHintSettings,
        visible_hints: impl IntoIterator<Item = Inlay>,
    ) -> ControlFlow<Option<InlaySplice>, Option<InlaySplice>> {
        let old_enabled = self.enabled;
        // If the setting for inlay hints has changed, update `enabled`. This condition avoids inlay
        // hint visibility changes when other settings change (such as theme).
        //
        // Another option might be to store whether the user has manually toggled inlay hint
        // visibility, and prefer this. This could lead to confusion as it means inlay hint
        // visibility would not change when updating the setting if they were ever toggled.
        if new_hint_settings.enabled != self.enabled_in_settings {
            self.enabled = new_hint_settings.enabled;
            self.enabled_in_settings = new_hint_settings.enabled;
            self.modifiers_override = false;
        };
        self.invalidate_debounce = debounce_value(new_hint_settings.edit_debounce_ms);
        self.append_debounce = debounce_value(new_hint_settings.scroll_debounce_ms);
        let new_allowed_hint_kinds = new_hint_settings.enabled_inlay_hint_kinds();
        match (old_enabled, self.enabled) {
            (false, false) => {
                self.allowed_hint_kinds = new_allowed_hint_kinds;
                ControlFlow::Break(None)
            }
            (true, true) => {
                if new_allowed_hint_kinds == self.allowed_hint_kinds {
                    ControlFlow::Break(None)
                } else {
                    self.allowed_hint_kinds = new_allowed_hint_kinds;
                    ControlFlow::Continue(
                        Some(InlaySplice {
                            to_remove: visible_hints
                                .into_iter()
                                .filter_map(|inlay| {
                                    let inlay_kind = self.added_hints.get(&inlay.id).copied()?;
                                    if !self.allowed_hint_kinds.contains(&inlay_kind) {
                                        Some(inlay.id)
                                    } else {
                                        None
                                    }
                                })
                                .collect(),
                            to_insert: Vec::new(),
                        })
                        .filter(|splice| !splice.is_empty()),
                    )
                }
            }
            (true, false) => {
                self.modifiers_override = false;
                self.allowed_hint_kinds = new_allowed_hint_kinds;
                let mut visible_hints = visible_hints.into_iter().peekable();
                if visible_hints.peek().is_none() {
                    ControlFlow::Break(None)
                } else {
                    self.clear();
                    ControlFlow::Break(Some(InlaySplice {
                        to_remove: visible_hints.map(|inlay| inlay.id).collect(),
                        to_insert: Vec::new(),
                    }))
                }
            }
            (false, true) => {
                self.modifiers_override = false;
                self.allowed_hint_kinds = new_allowed_hint_kinds;
                ControlFlow::Continue(
                    Some(InlaySplice {
                        to_remove: visible_hints
                            .into_iter()
                            .filter_map(|inlay| {
                                let inlay_kind = self.added_hints.get(&inlay.id).copied()?;
                                if !self.allowed_hint_kinds.contains(&inlay_kind) {
                                    Some(inlay.id)
                                } else {
                                    None
                                }
                            })
                            .collect(),
                        to_insert: Vec::new(),
                    })
                    .filter(|splice| !splice.is_empty()),
                )
            }
        }
    }

    pub(crate) fn remove_inlay_chunk_data<'a>(
        &'a mut self,
        removed_buffer_ids: impl IntoIterator<Item = &'a BufferId> + 'a,
    ) {
        for buffer_id in removed_buffer_ids {
            self.hint_refresh_tasks.remove(buffer_id);
            self.hint_chunk_fetching.remove(buffer_id);
        }
    }
}

#[derive(Debug, Clone)]
pub enum InlayHintRefreshReason {
    ModifiersChanged(bool),
    Toggle(bool),
    SettingsChange(InlayHintSettings),
    NewLinesShown,
    BufferEdited(BufferId),
    ServerRemoved,
    LanguageServerRegistered,
    RefreshRequested { server_id: LanguageServerId },
    BuffersRemoved(Vec<BufferId>),
}

impl Editor {
    pub fn supports_inlay_hints(&self, cx: &mut App) -> bool {
        let Some(provider) = self.semantics_provider.as_ref() else {
            return false;
        };

        let mut supports = false;
        self.buffer().update(cx, |this, cx| {
            this.for_each_buffer(&mut |buffer| {
                supports |= provider.supports_inlay_hints(buffer, cx);
            });
        });

        supports
    }

    pub fn toggle_inline_values(
        &mut self,
        _: &ToggleInlineValues,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.inline_value_cache.enabled = !self.inline_value_cache.enabled;

        self.refresh_inline_values(cx);
    }

    pub fn toggle_inlay_hints(
        &mut self,
        _: &ToggleInlayHints,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.refresh_inlay_hints(
            InlayHintRefreshReason::Toggle(!self.inlay_hints_enabled()),
            cx,
        );
    }

    pub fn inlay_hints_enabled(&self) -> bool {
        self.inlay_hints.as_ref().is_some_and(|cache| cache.enabled)
    }

    /// Updates inlay hints for the visible ranges of the singleton buffer(s).
    /// Based on its parameters, either invalidates the previous data, or appends to it.
    pub(crate) fn refresh_inlay_hints(
        &mut self,
        reason: InlayHintRefreshReason,
        cx: &mut Context<Self>,
    ) {
        if !self.lsp_data_enabled() || self.inlay_hints.is_none() {
            return;
        }
        let Some(semantics_provider) = self.semantics_provider() else {
            return;
        };
        let Some(invalidate_cache) = self.refresh_editor_data(&reason, cx) else {
            return;
        };

        let debounce = match &reason {
            InlayHintRefreshReason::SettingsChange(_)
            | InlayHintRefreshReason::Toggle(_)
            | InlayHintRefreshReason::BuffersRemoved(_)
            | InlayHintRefreshReason::ModifiersChanged(_) => None,
            _may_need_lsp_call => self.inlay_hints.as_ref().and_then(|inlay_hints| {
                if invalidate_cache.should_invalidate() {
                    inlay_hints.invalidate_debounce
                } else {
                    inlay_hints.append_debounce
                }
            }),
        };

        let mut visible_excerpts = self.visible_buffer_ranges(cx);
        visible_excerpts.retain(|(snapshot, _, _)| self.is_lsp_relevant(snapshot.file(), cx));

        let mut invalidate_hints_for_buffers = HashSet::default();
        let ignore_previous_fetches = match reason {
            InlayHintRefreshReason::ModifiersChanged(_)
            | InlayHintRefreshReason::Toggle(_)
            | InlayHintRefreshReason::SettingsChange(_)
            | InlayHintRefreshReason::ServerRemoved
            | InlayHintRefreshReason::LanguageServerRegistered
            | InlayHintRefreshReason::RefreshRequested { .. } => true,
            InlayHintRefreshReason::NewLinesShown | InlayHintRefreshReason::BuffersRemoved(_) => {
                false
            }
            InlayHintRefreshReason::BufferEdited(buffer_id) => {
                let Some(affected_language) = self
                    .buffer()
                    .read(cx)
                    .buffer(buffer_id)
                    .and_then(|buffer| buffer.read(cx).language().cloned())
                else {
                    return;
                };

                invalidate_hints_for_buffers.extend(
                    self.buffer()
                        .read(cx)
                        .all_buffers()
                        .into_iter()
                        .filter_map(|buffer| {
                            let buffer = buffer.read(cx);
                            if buffer.language() == Some(&affected_language) {
                                Some(buffer.remote_id())
                            } else {
                                None
                            }
                        }),
                );

                semantics_provider.invalidate_inlay_hints(&invalidate_hints_for_buffers, cx);
                visible_excerpts.retain(|(buffer_snapshot, _, _)| {
                    buffer_snapshot.language() == Some(&affected_language)
                });
                false
            }
        };

        let multi_buffer = self.buffer().clone();

        let Some(inlay_hints) = self.inlay_hints.as_mut() else {
            return;
        };

        if invalidate_cache.should_invalidate()
            && !matches!(reason, InlayHintRefreshReason::RefreshRequested { .. })
        {
            if invalidate_hints_for_buffers.is_empty() {
                inlay_hints.clear();
            } else {
                inlay_hints.clear_for_buffers(
                    &invalidate_hints_for_buffers,
                    Self::visible_inlay_hints(self.display_map.read(cx)),
                    &multi_buffer.read(cx).snapshot(cx),
                );
            }
        }
        inlay_hints
            .invalidate_hints_for_buffers
            .extend(invalidate_hints_for_buffers);

        let mut buffers_to_query = HashMap::default();
        for (buffer_snapshot, visible_range, _) in visible_excerpts {
            let buffer_id = buffer_snapshot.remote_id();

            if !self.registered_buffers.contains_key(&buffer_id) {
                continue;
            }

            let Some(buffer) = multi_buffer.read(cx).buffer(buffer_id) else {
                continue;
            };

            let buffer_version = buffer_snapshot.version().clone();
            let buffer_anchor_range = buffer_snapshot.anchor_before(visible_range.start)
                ..buffer_snapshot.anchor_after(visible_range.end);

            let visible_excerpts =
                buffers_to_query
                    .entry(buffer_id)
                    .or_insert_with(|| VisibleExcerpts {
                        ranges: Vec::new(),
                        buffer_version: buffer_version.clone(),
                        buffer: buffer.clone(),
                    });
            visible_excerpts.buffer_version = buffer_version;
            visible_excerpts.ranges.push(buffer_anchor_range);
        }

        for (buffer_id, visible_excerpts) in buffers_to_query {
            let Some(buffer) = multi_buffer.read(cx).buffer(buffer_id) else {
                continue;
            };

            let (fetched_for_version, fetched_chunks) = inlay_hints
                .hint_chunk_fetching
                .entry(buffer_id)
                .or_default();
            if visible_excerpts
                .buffer_version
                .changed_since(fetched_for_version)
            {
                *fetched_for_version = visible_excerpts.buffer_version.clone();
                fetched_chunks.clear();
                inlay_hints.hint_refresh_tasks.remove(&buffer_id);
            }

            let known_chunks = if ignore_previous_fetches {
                None
            } else {
                Some((fetched_for_version.clone(), fetched_chunks.clone()))
            };

            let mut applicable_chunks =
                semantics_provider.applicable_inlay_chunks(&buffer, &visible_excerpts.ranges, cx);
            applicable_chunks.retain(|chunk| fetched_chunks.insert(chunk.clone()));
            if applicable_chunks.is_empty() && !ignore_previous_fetches {
                continue;
            }
            inlay_hints
                .hint_refresh_tasks
                .entry(buffer_id)
                .or_default()
                .push(spawn_editor_hints_refresh(
                    buffer_id,
                    invalidate_cache,
                    debounce,
                    visible_excerpts,
                    known_chunks,
                    applicable_chunks,
                    cx,
                ));
        }
    }

    pub fn clear_inlay_hints(&mut self, cx: &mut Context<Self>) {
        let to_remove = Self::visible_inlay_hints(self.display_map.read(cx))
            .map(|inlay| inlay.id)
            .collect::<Vec<_>>();
        self.splice_inlays(&to_remove, Vec::new(), cx);
    }

    fn refresh_editor_data(
        &mut self,
        reason: &InlayHintRefreshReason,
        cx: &mut Context<'_, Editor>,
    ) -> Option<InvalidationStrategy> {
        let Some(inlay_hints) = self.inlay_hints.as_mut() else {
            return None;
        };

        let invalidate_cache = match reason {
            InlayHintRefreshReason::ModifiersChanged(enabled) => {
                match inlay_hints.modifiers_override(*enabled) {
                    Some(enabled) => {
                        if enabled {
                            InvalidationStrategy::None
                        } else {
                            self.clear_inlay_hints(cx);
                            return None;
                        }
                    }
                    None => return None,
                }
            }
            InlayHintRefreshReason::Toggle(enabled) => {
                if inlay_hints.toggle(*enabled) {
                    if *enabled {
                        InvalidationStrategy::None
                    } else {
                        self.clear_inlay_hints(cx);
                        return None;
                    }
                } else {
                    return None;
                }
            }
            InlayHintRefreshReason::SettingsChange(new_settings) => {
                let visible_inlay_hints =
                    Self::visible_inlay_hints(self.display_map.read(cx)).collect::<Vec<_>>();
                match inlay_hints.update_settings(*new_settings, visible_inlay_hints) {
                    ControlFlow::Break(Some(InlaySplice {
                        to_remove,
                        to_insert,
                    })) => {
                        self.splice_inlays(&to_remove, to_insert, cx);
                        return None;
                    }
                    ControlFlow::Break(None) => return None,
                    ControlFlow::Continue(splice) => {
                        if let Some(InlaySplice {
                            to_remove,
                            to_insert,
                        }) = splice
                        {
                            self.splice_inlays(&to_remove, to_insert, cx);
                        }
                        InvalidationStrategy::None
                    }
                }
            }
            InlayHintRefreshReason::BuffersRemoved(buffers_removed) => {
                let to_remove = self
                    .display_map
                    .read(cx)
                    .current_inlays()
                    .filter_map(|inlay| {
                        let anchor = inlay.position.raw_text_anchor()?;
                        if buffers_removed.contains(&anchor.buffer_id) {
                            Some(inlay.id)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                self.splice_inlays(&to_remove, Vec::new(), cx);
                return None;
            }
            InlayHintRefreshReason::ServerRemoved => InvalidationStrategy::BufferEdited,
            InlayHintRefreshReason::NewLinesShown
            | InlayHintRefreshReason::LanguageServerRegistered => InvalidationStrategy::None,
            InlayHintRefreshReason::BufferEdited(_) => InvalidationStrategy::BufferEdited,
            InlayHintRefreshReason::RefreshRequested { server_id } => {
                InvalidationStrategy::RefreshRequested {
                    server_id: *server_id,
                }
            }
        };

        match &mut self.inlay_hints {
            Some(inlay_hints) => {
                if !inlay_hints.enabled
                    && !matches!(reason, InlayHintRefreshReason::ModifiersChanged(_))
                {
                    return None;
                }
            }
            None => return None,
        }

        Some(invalidate_cache)
    }

    fn visible_inlay_hints(display_map: &DisplayMap) -> impl Iterator<Item = Inlay> + use<'_> {
        display_map
            .current_inlays()
            .filter(move |inlay| matches!(inlay.id, InlayId::Hint(_)))
            .cloned()
    }

    pub fn update_inlay_link_and_hover_points(
        &mut self,
        snapshot: &EditorSnapshot,
        point_for_position: PointForPosition,
        mouse_position: Option<gpui::Point<Pixels>>,
        secondary_held: bool,
        shift_held: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(lsp_store) = self.project().map(|project| project.read(cx).lsp_store()) else {
            return;
        };
        let hovered_offset = if point_for_position.column_overshoot_after_line_end == 0 {
            Some(
                snapshot
                    .display_point_to_inlay_offset(point_for_position.exact_unclipped, Bias::Left),
            )
        } else {
            None
        };
        let mut go_to_definition_updated = false;
        let mut hover_updated = false;
        if let Some(hovered_offset) = hovered_offset {
            let buffer_snapshot = self.buffer().read(cx).snapshot(cx);
            let previous_valid_anchor = buffer_snapshot.anchor_at(
                point_for_position.previous_valid.to_point(snapshot),
                Bias::Left,
            );
            let next_valid_anchor = buffer_snapshot.anchor_at(
                point_for_position.next_valid.to_point(snapshot),
                Bias::Right,
            );
            if let Some(hovered_hint) = Self::visible_inlay_hints(self.display_map.read(cx))
                .filter(|hint| snapshot.can_resolve(&hint.position))
                .skip_while(|hint| {
                    hint.position
                        .cmp(&previous_valid_anchor, &buffer_snapshot)
                        .is_lt()
                })
                .take_while(|hint| {
                    hint.position
                        .cmp(&next_valid_anchor, &buffer_snapshot)
                        .is_le()
                })
                .max_by_key(|hint| hint.id)
            {
                if let Some(ResolvedHint::Resolved(cached_hint)) = buffer_snapshot
                    .anchor_to_buffer_anchor(hovered_hint.position)
                    .and_then(|(anchor, _)| {
                        lsp_store.update(cx, |lsp_store, cx| {
                            lsp_store.resolved_hint(anchor.buffer_id, hovered_hint.id, cx)
                        })
                    })
                {
                    match cached_hint.resolve_state {
                        ResolveState::Resolved => {
                            let original_text = cached_hint.text();
                            let actual_left_padding =
                                if cached_hint.padding_left && !original_text.starts_with(" ") {
                                    1
                                } else {
                                    0
                                };
                            let actual_right_padding =
                                if cached_hint.padding_right && !original_text.ends_with(" ") {
                                    1
                                } else {
                                    0
                                };
                            match cached_hint.label {
                                InlayHintLabel::String(_) => {
                                    if let Some(tooltip) = cached_hint.tooltip {
                                        hover_popover::hover_at_inlay(
                                            self,
                                            InlayHover {
                                                tooltip: match tooltip {
                                                    InlayHintTooltip::String(text) => HoverBlock {
                                                        text,
                                                        kind: HoverBlockKind::PlainText,
                                                    },
                                                    InlayHintTooltip::MarkupContent(content) => {
                                                        HoverBlock {
                                                            text: content.value,
                                                            kind: content.kind,
                                                        }
                                                    }
                                                },
                                                range: InlayHighlight {
                                                    inlay: hovered_hint.id,
                                                    inlay_position: hovered_hint.position,
                                                    range: actual_left_padding
                                                        ..hovered_hint.text().len()
                                                            - actual_right_padding,
                                                },
                                            },
                                            window,
                                            cx,
                                        );
                                        hover_updated = true;
                                    }
                                }
                                InlayHintLabel::LabelParts(label_parts) => {
                                    let hint_start =
                                        snapshot.anchor_to_inlay_offset(hovered_hint.position);
                                    let content_start =
                                        InlayOffset(hint_start.0 + actual_left_padding);
                                    if let Some((hovered_hint_part, part_range)) =
                                        hover_popover::find_hovered_hint_part(
                                            label_parts,
                                            content_start,
                                            hovered_offset,
                                        )
                                    {
                                        let highlight_start = part_range.start - hint_start;
                                        let highlight_end = part_range.end - hint_start;
                                        let highlight = InlayHighlight {
                                            inlay: hovered_hint.id,
                                            inlay_position: hovered_hint.position,
                                            range: highlight_start..highlight_end,
                                        };
                                        if let Some(tooltip) = hovered_hint_part.tooltip {
                                            hover_popover::hover_at_inlay(
                                                self,
                                                InlayHover {
                                                    tooltip: match tooltip {
                                                        InlayHintLabelPartTooltip::String(text) => {
                                                            HoverBlock {
                                                                text,
                                                                kind: HoverBlockKind::PlainText,
                                                            }
                                                        }
                                                        InlayHintLabelPartTooltip::MarkupContent(
                                                            content,
                                                        ) => HoverBlock {
                                                            text: content.value,
                                                            kind: content.kind,
                                                        },
                                                    },
                                                    range: highlight.clone(),
                                                },
                                                window,
                                                cx,
                                            );
                                            hover_updated = true;
                                        }
                                        if let Some((language_server_id, location)) =
                                            hovered_hint_part.location
                                            && secondary_held
                                            && !self.has_pending_nonempty_selection()
                                        {
                                            go_to_definition_updated = true;
                                            show_link_definition(
                                                shift_held,
                                                self,
                                                TriggerPoint::InlayHint(
                                                    highlight,
                                                    location,
                                                    language_server_id,
                                                ),
                                                snapshot,
                                                window,
                                                cx,
                                            );
                                        }
                                    }
                                }
                            };
                        }
                        ResolveState::CanResolve(_, _) => debug_panic!(
                            "Expected resolved_hint retrieval to return a resolved hint"
                        ),
                        ResolveState::Resolving => {}
                    }
                }
            }
        }

        if !go_to_definition_updated {
            self.hide_hovered_link(cx)
        }
        if !hover_updated {
            hover_popover::hover_at(self, None, mouse_position, window, cx);
        }
    }

    fn inlay_hints_for_buffer(
        &mut self,
        invalidate_cache: InvalidationStrategy,
        buffer_excerpts: VisibleExcerpts,
        known_chunks: Option<(Global, HashSet<Range<BufferRow>>)>,
        cx: &mut Context<Self>,
    ) -> Option<Vec<Task<(Range<BufferRow>, anyhow::Result<CacheInlayHints>)>>> {
        let semantics_provider = self.semantics_provider()?;

        let new_hint_tasks = semantics_provider
            .inlay_hints(
                invalidate_cache,
                buffer_excerpts.buffer,
                buffer_excerpts.ranges,
                known_chunks,
                cx,
            )
            .unwrap_or_default();

        let mut hint_tasks = None;
        for (row_range, new_hints_task) in new_hint_tasks {
            hint_tasks
                .get_or_insert_with(Vec::new)
                .push(cx.spawn(async move |_, _| (row_range, new_hints_task.await)));
        }
        hint_tasks
    }

    fn apply_fetched_hints(
        &mut self,
        buffer_id: BufferId,
        query_version: Global,
        invalidate_cache: InvalidationStrategy,
        new_hints: Vec<(Range<BufferRow>, anyhow::Result<CacheInlayHints>)>,
        cx: &mut Context<Self>,
    ) {
        let multi_buffer_snapshot = self.buffer.read(cx).snapshot(cx);
        let visible_inlay_hint_ids = Self::visible_inlay_hints(self.display_map.read(cx))
            .filter(|inlay| {
                multi_buffer_snapshot
                    .anchor_to_buffer_anchor(inlay.position)
                    .map(|(anchor, _)| anchor.buffer_id)
                    == Some(buffer_id)
            })
            .map(|inlay| inlay.id)
            .collect::<Vec<_>>();
        let Some(inlay_hints) = &mut self.inlay_hints else {
            return;
        };
        let Some(buffer_snapshot) = self
            .buffer
            .read(cx)
            .buffer(buffer_id)
            .map(|buffer| buffer.read(cx).snapshot())
        else {
            return;
        };

        let mut hints_to_remove = Vec::new();

        // If we've received hints from the cache, it means `invalidate_cache` had invalidated whatever possible there,
        // and most probably there are no more hints with IDs from `visible_inlay_hint_ids` in the cache.
        // So, if we hover such hints, no resolve will happen.
        //
        // Another issue is in the fact that changing one buffer may lead to other buffers' hints changing, so more cache entries may be removed.
        // Hence, clear all excerpts' hints in the multi buffer: later, the invalidated ones will re-trigger the LSP query, the rest will be restored
        // from the cache.
        if invalidate_cache.should_invalidate() {
            for hint_id in &visible_inlay_hint_ids {
                inlay_hints.added_hints.remove(hint_id);
            }
            hints_to_remove.extend(visible_inlay_hint_ids);

            // When invalidating, this task removes ALL visible hints for the buffer
            // but only adds back hints for its own chunk ranges. Chunks fetched by
            // other concurrent tasks (e.g., a scroll task that completed before this
            // edit task) would have their hints removed but remain marked as "already
            // fetched" in hint_chunk_fetching, preventing re-fetch on the next
            // NewLinesShown. Fix: retain only chunks that this task has results for.
            let task_chunk_ranges: HashSet<&Range<BufferRow>> =
                new_hints.iter().map(|(range, _)| range).collect();
            if let Some((_, fetched_chunks)) = inlay_hints.hint_chunk_fetching.get_mut(&buffer_id) {
                fetched_chunks.retain(|chunk| task_chunk_ranges.contains(chunk));
            }
        }

        let mut inserted_hint_text = HashMap::default();
        let new_hints = new_hints
            .into_iter()
            .filter_map(|(chunk_range, hints_result)| {
                let chunks_fetched = inlay_hints.hint_chunk_fetching.get_mut(&buffer_id);
                match hints_result {
                    Ok(new_hints) => {
                        if new_hints.is_empty() {
                            if let Some((_, chunks_fetched)) = chunks_fetched {
                                chunks_fetched.remove(&chunk_range);
                            }
                        }
                        Some(new_hints)
                    }
                    Err(e) => {
                        log::error!(
                            "查询缓冲区行范围 {chunk_range:?} 的内联提示失败,{e:#}"
                        );
                        if let Some((for_version, chunks_fetched)) = chunks_fetched {
                            if for_version == &query_version {
                                chunks_fetched.remove(&chunk_range);
                            }
                        }
                        None
                    }
                }
            })
            .flat_map(|new_hints| {
                let mut hints_deduplicated = Vec::new();

                if new_hints.len() > 1 {
                    for (server_id, new_hints) in new_hints {
                        for (new_id, new_hint) in new_hints {
                            let hints_text_for_position = inserted_hint_text
                                .entry(new_hint.position)
                                .or_insert_with(HashMap::default);
                            let insert =
                                match hints_text_for_position.entry(new_hint.text().to_string()) {
                                    hash_map::Entry::Occupied(o) => o.get() == &server_id,
                                    hash_map::Entry::Vacant(v) => {
                                        v.insert(server_id);
                                        true
                                    }
                                };

                            if insert {
                                hints_deduplicated.push((new_id, new_hint));
                            }
                        }
                    }
                } else {
                    hints_deduplicated.extend(new_hints.into_values().flatten());
                }

                hints_deduplicated
            })
            .filter(|(hint_id, lsp_hint)| {
                inlay_hints.allowed_hint_kinds.contains(&lsp_hint.kind)
                    && inlay_hints
                        .added_hints
                        .insert(*hint_id, lsp_hint.kind)
                        .is_none()
            })
            .sorted_by(|(_, a), (_, b)| a.position.cmp(&b.position, &buffer_snapshot))
            .collect::<Vec<_>>();

        let hints_to_insert = multi_buffer_snapshot
            .text_anchors_to_visible_anchors(
                new_hints.iter().map(|(_, lsp_hint)| lsp_hint.position),
            )
            .into_iter()
            .zip(&new_hints)
            .filter_map(|(position, (hint_id, hint))| Some(Inlay::hint(*hint_id, position?, &hint)))
            .collect();
        let invalidate_hints_for_buffers =
            std::mem::take(&mut inlay_hints.invalidate_hints_for_buffers);
        if !invalidate_hints_for_buffers.is_empty() {
            hints_to_remove.extend(
                Self::visible_inlay_hints(self.display_map.read(cx))
                    .filter(|inlay| {
                        multi_buffer_snapshot
                            .anchor_to_buffer_anchor(inlay.position)
                            .is_none_or(|(anchor, _)| {
                                invalidate_hints_for_buffers.contains(&anchor.buffer_id)
                            })
                    })
                    .map(|inlay| inlay.id),
            );
        }

        self.splice_inlays(&hints_to_remove, hints_to_insert, cx);
    }
}

#[derive(Debug)]
struct VisibleExcerpts {
    ranges: Vec<Range<text::Anchor>>,
    buffer_version: Global,
    buffer: Entity<language::Buffer>,
}

fn spawn_editor_hints_refresh(
    buffer_id: BufferId,
    invalidate_cache: InvalidationStrategy,
    debounce: Option<Duration>,
    buffer_excerpts: VisibleExcerpts,
    known_chunks: Option<(Global, HashSet<Range<BufferRow>>)>,
    applicable_chunks: Vec<Range<BufferRow>>,
    cx: &mut Context<'_, Editor>,
) -> Task<()> {
    cx.spawn(async move |editor, cx| {
        if let Some(debounce) = debounce {
            cx.background_executor().timer(debounce).await;
        }

        let query_version = buffer_excerpts.buffer_version.clone();
        let Some(hint_tasks) = editor
            .update(cx, |editor, cx| {
                editor.inlay_hints_for_buffer(invalidate_cache, buffer_excerpts, known_chunks, cx)
            })
            .ok()
        else {
            return;
        };
        let hint_tasks = hint_tasks.unwrap_or_default();
        if hint_tasks.is_empty() {
            editor
                .update(cx, |editor, _| {
                    if let Some((_, hint_chunk_fetching)) = editor
                        .inlay_hints
                        .as_mut()
                        .and_then(|inlay_hints| inlay_hints.hint_chunk_fetching.get_mut(&buffer_id))
                    {
                        for applicable_chunks in &applicable_chunks {
                            hint_chunk_fetching.remove(applicable_chunks);
                        }
                    }
                })
                .ok();
            return;
        }
        let new_hints = join_all(hint_tasks).await;
        editor
            .update(cx, |editor, cx| {
                editor.apply_fetched_hints(
                    buffer_id,
                    query_version,
                    invalidate_cache,
                    new_hints,
                    cx,
                );
            })
            .ok();
    })
}

#[cfg(test)]
pub mod tests {
    use crate::editor_tests::update_test_language_settings;
    use crate::inlays::inlay_hints::InlayHintRefreshReason;
    use crate::scroll::Autoscroll;
    use crate::scroll::ScrollAmount;
    use crate::{Editor, SelectionEffects};
    use collections::HashSet;
    use futures::channel::oneshot;
    use futures::{StreamExt, future};
    use gpui::{AppContext as _, Context, TestAppContext, WindowHandle};
    use itertools::Itertools as _;
    use language::language_settings::InlayHintKind;
    use language::{Capability, FakeLspAdapter};
    use language::{Language, LanguageConfig, LanguageMatcher};
    use languages::rust_lang;
    use lsp::{DEFAULT_LSP_REQUEST_TIMEOUT, FakeLanguageServer};
    use multi_buffer::{MultiBuffer, MultiBufferOffset, PathKey};
    use parking_lot::Mutex;
    use pretty_assertions::assert_eq;
    use project::{FakeFs, InvalidationStrategy, Project};
    use serde_json::json;
    use settings::{AllLanguageSettingsContent, InlayHintSettingsContent, SettingsStore};
    use std::ops::Range;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
    use std::time::Duration;
    use text::{OffsetRangeExt, Point};
    use ui::App;
    use util::path;
    use util::paths::natural_sort;

    #[gpui::test]
    async fn test_basic_cache_update_with_duplicate_hints(cx: &mut gpui::TestAppContext) {
        let allowed_hint_kinds = HashSet::from_iter([None, Some(InlayHintKind::Type)]);
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(allowed_hint_kinds.contains(&Some(InlayHintKind::Type))),
                show_parameter_hints: Some(
                    allowed_hint_kinds.contains(&Some(InlayHintKind::Parameter)),
                ),
                show_other_hints: Some(allowed_hint_kinds.contains(&None)),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });
        let (_, editor, fake_server) = prepare_test_objects(cx, |fake_server, file_with_hints| {
            let lsp_request_count = Arc::new(AtomicU32::new(0));
            fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                move |params, _| {
                    let task_lsp_request_count = Arc::clone(&lsp_request_count);
                    async move {
                        let i = task_lsp_request_count.fetch_add(1, Ordering::Release) + 1;
                        assert_eq!(
                            params.text_document.uri,
                            lsp::Uri::from_file_path(file_with_hints).unwrap(),
                        );
                        Ok(Some(vec![lsp::InlayHint {
                            position: lsp::Position::new(0, i),
                            label: lsp::InlayHintLabel::String(i.to_string()),
                            kind: None,
                            text_edits: None,
                            tooltip: None,
                            padding_left: None,
                            padding_right: None,
                            data: None,
                        }]))
                    }
                },
            );
        })
        .await;
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "打开编辑器时应获取其首批提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
                assert_eq!(
                    allowed_hint_kinds_for_editor(editor),
                    allowed_hint_kinds,
                    "缓存应使用编辑器设置来获取允许的提示类型"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                    s.select_ranges([MultiBufferOffset(13)..MultiBufferOffset(13)])
                });
                editor.handle_input("some change", window, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["2".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "编辑后应获取新的提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
                assert_eq!(
                    allowed_hint_kinds_for_editor(editor),
                    allowed_hint_kinds,
                    "缓存应使用编辑器设置来获取允许的提示类型"
                );
            })
            .unwrap();

        fake_server
            .request::<lsp::request::InlayHintRefreshRequest>((), DEFAULT_LSP_REQUEST_TIMEOUT)
            .await
            .into_response()
            .expect("内联提示刷新请求失败");
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["3".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "提示刷新/请求后应获取新的提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
                assert_eq!(
                    allowed_hint_kinds_for_editor(editor),
                    allowed_hint_kinds,
                    "缓存应使用编辑器设置来获取允许的提示类型"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_racy_cache_updates(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                ..InlayHintSettingsContent::default()
            })
        });
        let (_, editor, fake_server) = prepare_test_objects(cx, |fake_server, file_with_hints| {
            let lsp_request_count = Arc::new(AtomicU32::new(0));
            fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                move |params, _| {
                    let task_lsp_request_count = Arc::clone(&lsp_request_count);
                    async move {
                        let i = task_lsp_request_count.fetch_add(1, Ordering::Release) + 1;
                        assert_eq!(
                            params.text_document.uri,
                            lsp::Uri::from_file_path(file_with_hints).unwrap(),
                        );
                        Ok(Some(vec![lsp::InlayHint {
                            position: lsp::Position::new(0, i),
                            label: lsp::InlayHintLabel::String(i.to_string()),
                            kind: Some(lsp::InlayHintKind::TYPE),
                            text_edits: None,
                            tooltip: None,
                            padding_left: None,
                            padding_right: None,
                            data: None,
                        }]))
                    }
                },
            );
        })
        .await;
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "打开编辑器时应获取其首批提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        // Emulate simultaneous events: both editing, refresh and, slightly after, scroll updates are triggered.
        editor
            .update(cx, |editor, window, cx| {
                editor.handle_input("foo", window, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(5));
        let refresh_request = fake_server
            .request::<lsp::request::InlayHintRefreshRequest>((), lsp::DEFAULT_LSP_REQUEST_TIMEOUT);
        cx.executor().advance_clock(Duration::from_millis(5));
        editor
            .update(cx, |editor, _window, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.executor().run_until_parked();
        refresh_request.await.into_response().unwrap();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["2".to_string()];
                assert_eq!(expected_hints, cached_hint_labels(editor, cx), "尽管有多个同时发生的刷新,只应发出一次内联提示查询");
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_no_hint_duplication_when_refresh_races_with_fetch(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                ..InlayHintSettingsContent::default()
            })
        });
        let (first_request_unblock, first_request_gate) = oneshot::channel::<()>();
        let first_request_gate = Arc::new(Mutex::new(Some(first_request_gate)));
        let lsp_request_count = Arc::new(AtomicU32::new(0));
        let (_, editor, fake_server) = prepare_test_objects(cx, {
            let first_request_gate = first_request_gate.clone();
            let lsp_request_count = lsp_request_count.clone();
            move |fake_server, file_with_hints| {
                let lsp_request_count = lsp_request_count.clone();
                let first_request_gate = first_request_gate.clone();
                fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                    move |params, _| {
                        let first_request_gate = first_request_gate.lock().take();
                        let i = lsp_request_count.fetch_add(1, Ordering::Release) + 1;
                        async move {
                            if let Some(first_request_gate) = first_request_gate {
                                first_request_gate.await.ok();
                            }
                            assert_eq!(
                                params.text_document.uri,
                                lsp::Uri::from_file_path(file_with_hints).unwrap(),
                            );
                            Ok(Some(vec![lsp::InlayHint {
                                position: lsp::Position::new(0, 1),
                                label: lsp::InlayHintLabel::String(i.to_string()),
                                kind: Some(lsp::InlayHintKind::TYPE),
                                text_edits: None,
                                tooltip: None,
                                padding_left: None,
                                padding_right: None,
                                data: None,
                            }]))
                        }
                    },
                );
            }
        })
        .await;
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                assert!(
                    cached_hint_labels(editor, cx).is_empty(),
                    "初始提示获取被阻止,不应已填充缓存"
                );
            })
            .unwrap();

        // Emulate a server refresh request arriving while the initial fetch is still running.
        fake_server
            .request::<lsp::request::InlayHintRefreshRequest>((), lsp::DEFAULT_LSP_REQUEST_TIMEOUT)
            .await
            .into_response()
            .unwrap();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.executor().run_until_parked();

        first_request_unblock.send(()).unwrap();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.executor().run_until_parked();
        assert_eq!(
            2,
            lsp_request_count.load(Ordering::Acquire),
            "刷新应已重新查询服务器"
        );

        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["2".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "与进行中的获取竞争的刷新应替换其提示,而不是重复它们"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_cache_update_on_lsp_completion_tasks(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let (_, editor, fake_server) = prepare_test_objects(cx, |fake_server, file_with_hints| {
            let lsp_request_count = Arc::new(AtomicU32::new(0));
            fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                move |params, _| {
                    let task_lsp_request_count = Arc::clone(&lsp_request_count);
                    async move {
                        assert_eq!(
                            params.text_document.uri,
                            lsp::Uri::from_file_path(file_with_hints).unwrap(),
                        );
                        let current_call_id =
                            Arc::clone(&task_lsp_request_count).fetch_add(1, Ordering::SeqCst);
                        Ok(Some(vec![lsp::InlayHint {
                            position: lsp::Position::new(0, current_call_id),
                            label: lsp::InlayHintLabel::String(current_call_id.to_string()),
                            kind: None,
                            text_edits: None,
                            tooltip: None,
                            padding_left: None,
                            padding_right: None,
                            data: None,
                        }]))
                    }
                },
            );
        })
        .await;
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["0".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "打开编辑器时应获取其首批提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        let progress_token = 42;
        fake_server
            .request::<lsp::request::WorkDoneProgressCreate>(
                lsp::WorkDoneProgressCreateParams {
                    token: lsp::ProgressToken::Number(progress_token),
                },
                DEFAULT_LSP_REQUEST_TIMEOUT,
            )
            .await
            .into_response()
            .expect("工作完成进度创建请求失败");
        cx.executor().run_until_parked();
        fake_server.notify::<lsp::notification::Progress>(lsp::ProgressParams {
            token: lsp::ProgressToken::Number(progress_token),
            value: lsp::ProgressParamsValue::WorkDone(lsp::WorkDoneProgress::Begin(
                lsp::WorkDoneProgressBegin::default(),
            )),
        });
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["0".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "工作任务运行时不应更新提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        fake_server.notify::<lsp::notification::Progress>(lsp::ProgressParams {
            token: lsp::ProgressToken::Number(progress_token),
            value: lsp::ProgressParamsValue::WorkDone(lsp::WorkDoneProgress::End(
                lsp::WorkDoneProgressEnd::default(),
            )),
        });
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "工作任务完成后应查询新的提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        run_work_cycle(&fake_server, progress_token + 1, cx).await;

        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "没有缓冲区更改的重复工作周期不应再次使提示失效"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.handle_input("~", window, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["2".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "缓冲区编辑应使提示失效并重新查询"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        run_work_cycle(&fake_server, progress_token + 2, cx).await;

        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["3".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "缓冲区编辑应重新允许工作结束时的提示刷新"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_no_hint_updates_for_unrelated_language_files(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": "fn main() { a } // and some long comment to ensure inlays are not trimmed out",
                "other.md": "Test md file with some text",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;

        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        let mut rs_fake_servers = None;
        let mut md_fake_servers = None;
        for (name, path_suffix) in [("Rust", "rs"), ("Markdown", "md")] {
            language_registry.add(Arc::new(Language::new(
                LanguageConfig {
                    name: name.into(),
                    matcher: (LanguageMatcher {
                        path_suffixes: vec![path_suffix.to_string()],
                        ..Default::default()
                    })
                    .into(),
                    ..Default::default()
                },
                Some(tree_sitter_rust::LANGUAGE.into()),
            )));
            let fake_servers = language_registry.register_fake_lsp(
                name,
                FakeLspAdapter {
                    name,
                    capabilities: lsp::ServerCapabilities {
                        inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                        ..Default::default()
                    },
                    initializer: Some(Box::new({
                        move |fake_server| {
                            let rs_lsp_request_count = Arc::new(AtomicU32::new(0));
                            let md_lsp_request_count = Arc::new(AtomicU32::new(0));
                            fake_server
                                .set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                                    move |params, _| {
                                        let i = match name {
                                            "Rust" => {
                                                assert_eq!(
                                                    params.text_document.uri,
                                                    lsp::Uri::from_file_path(path!("/a/main.rs"))
                                                        .unwrap(),
                                                );
                                                rs_lsp_request_count.fetch_add(1, Ordering::Release)
                                                    + 1
                                            }
                                            "Markdown" => {
                                                assert_eq!(
                                                    params.text_document.uri,
                                                    lsp::Uri::from_file_path(path!("/a/other.md"))
                                                        .unwrap(),
                                                );
                                                md_lsp_request_count.fetch_add(1, Ordering::Release)
                                                    + 1
                                            }
                                            unexpected => {
                                                panic!("意外的语言:{unexpected}")
                                            }
                                        };

                                        async move {
                                            let query_start = params.range.start;
                                            Ok(Some(vec![lsp::InlayHint {
                                                position: query_start,
                                                label: lsp::InlayHintLabel::String(i.to_string()),
                                                kind: None,
                                                text_edits: None,
                                                tooltip: None,
                                                padding_left: None,
                                                padding_right: None,
                                                data: None,
                                            }]))
                                        }
                                    },
                                );
                        }
                    })),
                    ..Default::default()
                },
            );
            match name {
                "Rust" => rs_fake_servers = Some(fake_servers),
                "Markdown" => md_fake_servers = Some(fake_servers),
                _ => unreachable!(),
            }
        }

        let rs_buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let rs_editor = cx.add_window(|window, cx| {
            Editor::for_buffer(rs_buffer, Some(project.clone()), window, cx)
        });
        cx.executor().run_until_parked();

        let _rs_fake_server = rs_fake_servers.unwrap().next().await.unwrap();
        cx.executor().run_until_parked();

        // Establish a viewport so the editor considers itself visible and the hint refresh
        // pipeline runs. Then explicitly trigger a refresh.
        rs_editor
            .update(cx, |editor, window, cx| {
                editor.set_visible_line_count(50.0, window, cx);
                editor.set_visible_column_count(120.0);
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        rs_editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "打开编辑器时应获取其首批提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        cx.executor().run_until_parked();
        let md_buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/other.md"), cx)
            })
            .await
            .unwrap();
        let md_editor =
            cx.add_window(|window, cx| Editor::for_buffer(md_buffer, Some(project), window, cx));
        cx.executor().run_until_parked();

        let _md_fake_server = md_fake_servers.unwrap().next().await.unwrap();
        cx.executor().run_until_parked();

        // Establish a viewport so the editor considers itself visible and the hint refresh
        // pipeline runs. Then explicitly trigger a refresh.
        md_editor
            .update(cx, |editor, window, cx| {
                editor.set_visible_line_count(50.0, window, cx);
                editor.set_visible_column_count(120.0);
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        md_editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "Markdown 编辑器应有独立的版本,重复 Rust 编辑器的规则"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        rs_editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                    s.select_ranges([MultiBufferOffset(13)..MultiBufferOffset(13)])
                });
                editor.handle_input("some rs change", window, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        rs_editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["2".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "编辑后 Rust 内联提示缓存应发生变化"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
        md_editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "Markdown 编辑器不应受 Rust 编辑器更改的影响"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        md_editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                    s.select_ranges([MultiBufferOffset(13)..MultiBufferOffset(13)])
                });
                editor.handle_input("some md change", window, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        md_editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["2".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "Rust 编辑器不应受 Markdown 编辑器更改的影响"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
        rs_editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec!["2".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "Markdown 编辑器也应独立变化"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_hint_setting_changes(cx: &mut gpui::TestAppContext) {
        let allowed_hint_kinds = HashSet::from_iter([None, Some(InlayHintKind::Type)]);
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(allowed_hint_kinds.contains(&Some(InlayHintKind::Type))),
                show_parameter_hints: Some(
                    allowed_hint_kinds.contains(&Some(InlayHintKind::Parameter)),
                ),
                show_other_hints: Some(allowed_hint_kinds.contains(&None)),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let lsp_request_count = Arc::new(AtomicUsize::new(0));
        let (_, editor, fake_server) = prepare_test_objects(cx, {
            let lsp_request_count = lsp_request_count.clone();
            move |fake_server, file_with_hints| {
                let lsp_request_count = lsp_request_count.clone();
                fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                    move |params, _| {
                        lsp_request_count.fetch_add(1, Ordering::Release);
                        async move {
                            assert_eq!(
                                params.text_document.uri,
                                lsp::Uri::from_file_path(file_with_hints).unwrap(),
                            );
                            Ok(Some(vec![
                                lsp::InlayHint {
                                    position: lsp::Position::new(0, 1),
                                    label: lsp::InlayHintLabel::String("类型提示".to_string()),
                                    kind: Some(lsp::InlayHintKind::TYPE),
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: None,
                                    data: None,
                                },
                                lsp::InlayHint {
                                    position: lsp::Position::new(0, 2),
                                    label: lsp::InlayHintLabel::String(
                                        "参数提示".to_string(),
                                    ),
                                    kind: Some(lsp::InlayHintKind::PARAMETER),
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: None,
                                    data: None,
                                },
                                lsp::InlayHint {
                                    position: lsp::Position::new(0, 3),
                                    label: lsp::InlayHintLabel::String("其他提示".to_string()),
                                    kind: None,
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: None,
                                    data: None,
                                },
                            ]))
                        }
                    },
                );
            }
        })
        .await;
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    1,
                    "应查询一次新提示"
                );
                assert_eq!(
                    vec![
                        "类型提示".to_string(),
                        "参数提示".to_string(),
                        "其他提示".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                    "打开编辑器时应获取其首批提示"
                );
                assert_eq!(
                    vec!["类型提示".to_string(), "其他提示".to_string()],
                    visible_hint_labels(editor, cx)
                );
                assert_eq!(
                    allowed_hint_kinds_for_editor(editor),
                    allowed_hint_kinds,
                    "缓存应使用编辑器设置来获取允许的提示类型"
                );
            })
            .unwrap();

        fake_server
            .request::<lsp::request::InlayHintRefreshRequest>((), DEFAULT_LSP_REQUEST_TIMEOUT)
            .await
            .into_response()
            .expect("内联提示刷新请求失败");
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    2,
                    "应加载两次新提示"
                );
                assert_eq!(
                    vec![
                        "类型提示".to_string(),
                        "参数提示".to_string(),
                        "其他提示".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                    "缓存的提示不应因允许的提示类型设置更新而改变"
                );
                assert_eq!(
                    vec!["类型提示".to_string(), "其他提示".to_string()],
                    visible_hint_labels(editor, cx)
                );
            })
            .unwrap();

        for (new_allowed_hint_kinds, expected_visible_hints) in [
            (HashSet::from_iter([None]), vec!["其他提示".to_string()]),
            (
                HashSet::from_iter([Some(InlayHintKind::Type)]),
                vec!["类型提示".to_string()],
            ),
            (
                HashSet::from_iter([Some(InlayHintKind::Parameter)]),
                vec!["参数提示".to_string()],
            ),
            (
                HashSet::from_iter([None, Some(InlayHintKind::Type)]),
                vec!["类型提示".to_string(), "其他提示".to_string()],
            ),
            (
                HashSet::from_iter([None, Some(InlayHintKind::Parameter)]),
                vec!["参数提示".to_string(), "其他提示".to_string()],
            ),
            (
                HashSet::from_iter([Some(InlayHintKind::Type), Some(InlayHintKind::Parameter)]),
                vec!["类型提示".to_string(), "参数提示".to_string()],
            ),
            (
                HashSet::from_iter([
                    None,
                    Some(InlayHintKind::Type),
                    Some(InlayHintKind::Parameter),
                ]),
                vec![
                    "类型提示".to_string(),
                    "参数提示".to_string(),
                    "其他提示".to_string(),
                ],
            ),
        ] {
            update_test_language_settings(cx, &|settings| {
                settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                    show_value_hints: Some(true),
                    enabled: Some(true),
                    edit_debounce_ms: Some(0),
                    scroll_debounce_ms: Some(0),
                    show_type_hints: Some(
                        new_allowed_hint_kinds.contains(&Some(InlayHintKind::Type)),
                    ),
                    show_parameter_hints: Some(
                        new_allowed_hint_kinds.contains(&Some(InlayHintKind::Parameter)),
                    ),
                    show_other_hints: Some(new_allowed_hint_kinds.contains(&None)),
                    show_background: Some(false),
                    toggle_on_modifiers_press: None,
                })
            });
            cx.executor().run_until_parked();
            editor.update(cx, |editor, _, cx| {
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    2,
                    "对于提示类型 {new_allowed_hint_kinds:?},在允许的提示类型更改时不应加载新提示"
                );
                assert_eq!(
                    vec![
                        "类型提示".to_string(),
                        "参数提示".to_string(),
                        "其他提示".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                    "对于提示类型 {new_allowed_hint_kinds:?},设置更改后应保持缓存的提示不变"
                );
                assert_eq!(
                    expected_visible_hints,
                    visible_hint_labels(editor, cx),
                    "对于提示类型 {new_allowed_hint_kinds:?},设置更改后应过滤可见提示"
                );
                assert_eq!(
                    allowed_hint_kinds_for_editor(editor),
                    new_allowed_hint_kinds,
                    "对于提示类型 {new_allowed_hint_kinds:?},缓存应使用编辑器设置来获取允许的提示类型"
                );
            }).unwrap();
        }

        let another_allowed_hint_kinds = HashSet::from_iter([Some(InlayHintKind::Type)]);
        update_test_language_settings(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(false),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(
                    another_allowed_hint_kinds.contains(&Some(InlayHintKind::Type)),
                ),
                show_parameter_hints: Some(
                    another_allowed_hint_kinds.contains(&Some(InlayHintKind::Parameter)),
                ),
                show_other_hints: Some(another_allowed_hint_kinds.contains(&None)),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    2,
                    "提示被禁用时不应加载新提示"
                );
                assert_eq!(
                    vec![
                        "类型提示".to_string(),
                        "参数提示".to_string(),
                        "其他提示".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                    "提示被禁用时不应清除缓存"
                );
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "提示被禁用时应清除可见提示"
                );
                assert_eq!(
                    allowed_hint_kinds_for_editor(editor),
                    another_allowed_hint_kinds,
                    "即使提示被禁用,也应更新其允许的提示类型"
                );
            })
            .unwrap();

        fake_server
            .request::<lsp::request::InlayHintRefreshRequest>((), DEFAULT_LSP_REQUEST_TIMEOUT)
            .await
            .into_response()
            .expect("内联提示刷新请求失败");
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    2,
                    "提示被禁用时不应加载新提示"
                );
                assert_eq!(
                    vec![
                        "类型提示".to_string(),
                        "参数提示".to_string(),
                        "其他提示".to_string(),
                    ],
                    cached_hint_labels(editor, cx)
                );
                assert_eq!(Vec::<String>::new(), visible_hint_labels(editor, cx));
            })
            .unwrap();

        let final_allowed_hint_kinds = HashSet::from_iter([Some(InlayHintKind::Parameter)]);
        update_test_language_settings(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(
                    final_allowed_hint_kinds.contains(&Some(InlayHintKind::Type)),
                ),
                show_parameter_hints: Some(
                    final_allowed_hint_kinds.contains(&Some(InlayHintKind::Parameter)),
                ),
                show_other_hints: Some(final_allowed_hint_kinds.contains(&None)),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    2,
                    "提示重新启用时不应查询新提示,因为文件版本未更改"
                );
                assert_eq!(
                    vec![
                        "类型提示".to_string(),
                        "参数提示".to_string(),
                        "其他提示".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                    "提示重新启用后,应完全重新填充其缓存的提示"
                );
                assert_eq!(
                    vec!["参数提示".to_string()],
                    visible_hint_labels(editor, cx),
                    "提示重新启用后,应重新填充并过滤其可见提示"
                );
                assert_eq!(
                    allowed_hint_kinds_for_editor(editor),
                    final_allowed_hint_kinds,
                    "提示重新启用时,缓存应更新编辑器设置"
                );
            })
            .unwrap();

        fake_server
            .request::<lsp::request::InlayHintRefreshRequest>((), DEFAULT_LSP_REQUEST_TIMEOUT)
            .await
            .into_response()
            .expect("内联提示刷新请求失败");
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    3,
                    "应再次查询新提示"
                );
                assert_eq!(
                    vec![
                        "类型提示".to_string(),
                        "参数提示".to_string(),
                        "其他提示".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                );
                assert_eq!(
                    vec!["参数提示".to_string()],
                    visible_hint_labels(editor, cx),
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_hint_request_cancellation(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let lsp_request_count = Arc::new(AtomicU32::new(0));
        let (_, editor, _) = prepare_test_objects(cx, {
            let lsp_request_count = lsp_request_count.clone();
            move |fake_server, file_with_hints| {
                let lsp_request_count = lsp_request_count.clone();
                fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                    move |params, _| {
                        let lsp_request_count = lsp_request_count.clone();
                        async move {
                            let i = lsp_request_count.fetch_add(1, Ordering::SeqCst) + 1;
                            assert_eq!(
                                params.text_document.uri,
                                lsp::Uri::from_file_path(file_with_hints).unwrap(),
                            );
                            Ok(Some(vec![lsp::InlayHint {
                                position: lsp::Position::new(0, i),
                                label: lsp::InlayHintLabel::String(i.to_string()),
                                kind: None,
                                text_edits: None,
                                tooltip: None,
                                padding_left: None,
                                padding_right: None,
                                data: None,
                            }]))
                        }
                    },
                );
            }
        })
        .await;

        let mut expected_changes = Vec::new();
        for change_after_opening in [
            "初始更改 #1",
            "初始更改 #2",
            "初始更改 #3",
        ] {
            editor
                .update(cx, |editor, window, cx| {
                    editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                        s.select_ranges([MultiBufferOffset(13)..MultiBufferOffset(13)])
                    });
                    editor.handle_input(change_after_opening, window, cx);
                })
                .unwrap();
            expected_changes.push(change_after_opening);
        }

        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                let current_text = editor.text(cx);
                for change in &expected_changes {
                    assert!(
                        current_text.contains(change),
                        "应应用所有已做的更改"
                    );
                }
                assert_eq!(
                    lsp_request_count.load(Ordering::Relaxed),
                    2,
                    "应查询两次新提示:一次用于编辑器初始化,一次用于中断所有其他编辑的最后一次编辑"
                );
                let expected_hints = vec!["2".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "应仅获取最后一次编辑产生的提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        let mut edits = Vec::new();
        for async_later_change in [
            "另一个更改 #1",
            "另一个更改 #2",
            "另一个更改 #3",
        ] {
            expected_changes.push(async_later_change);
            let task_editor = editor;
            edits.push(cx.spawn(|mut cx| async move {
                task_editor
                    .update(&mut cx, |editor, window, cx| {
                        editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                            s.select_ranges([MultiBufferOffset(13)..MultiBufferOffset(13)])
                        });
                        editor.handle_input(async_later_change, window, cx);
                    })
                    .unwrap();
            }));
        }
        let _ = future::join_all(edits).await;
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _, cx| {
                let current_text = editor.text(cx);
                for change in &expected_changes {
                    assert!(
                        current_text.contains(change),
                        "应应用所有已做的更改"
                    );
                }
                assert_eq!(
                    lsp_request_count.load(Ordering::SeqCst),
                    3,
                    "应再查询一次新提示,仅针对最后一次编辑"
                );
                let expected_hints = vec!["3".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "应仅获取最后一次编辑产生的提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_inlay_hint_response_after_buffer_shrinks(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                ..InlayHintSettingsContent::default()
            })
        });

        let (unblock_request, request_gate) = oneshot::channel::<()>();
        let request_gate = Arc::new(Mutex::new(Some(request_gate)));
        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": "let value = 1;\n".repeat(500),
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;
        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new({
                    let request_gate = request_gate.clone();
                    move |fake_server| {
                        let request_gate = request_gate.clone();
                        fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                            move |params, _| {
                                // Only block the request for the far-away chunk, so that any
                                // hint requests the editor makes for the top of the file do
                                // not consume the gate.
                                let request_gate = if params.range.start.line >= 400 {
                                    request_gate.lock().take()
                                } else {
                                    None
                                };
                                async move {
                                    if let Some(request_gate) = request_gate {
                                        request_gate.await.ok();
                                    }
                                    Ok(Some(Vec::new()))
                                }
                            },
                        );
                    }
                })),
                ..FakeLspAdapter::default()
            },
        );

        let buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        // An editor is only needed so that the language server starts for this buffer.
        let _editor = cx.add_window(|window, cx| {
            Editor::for_buffer(buffer.clone(), Some(project.clone()), window, cx)
        });
        cx.executor().run_until_parked();
        let _fake_server = fake_servers.next().await.unwrap();

        // Query a single chunk far into the buffer, so that its chunk id is well past the
        // number of chunks the buffer will have once shrunk.
        let hint_tasks = project.update(cx, |project, cx| {
            let query_range = buffer.read_with(cx, |buffer, _| {
                buffer.anchor_before(Point::new(460, 0))..buffer.anchor_after(Point::new(460, 0))
            });
            project.lsp_store().update(cx, |lsp_store, cx| {
                lsp_store.inlay_hints(
                    InvalidationStrategy::None,
                    buffer.clone(),
                    vec![query_range],
                    None,
                    cx,
                )
            })
        });
        assert_eq!(
            hint_tasks.len(),
            1,
            "应已为精确查询的块启动提示获取"
        );
        cx.executor().run_until_parked();
        assert!(
            request_gate.lock().is_none(),
            "语言服务器应已收到内联提示请求"
        );

        // Shrink the buffer while the response is still in flight, so that the buffer has
        // far fewer chunks than the pending request's chunk id.
        buffer.update(cx, |buffer, cx| {
            let entire_buffer = 0..buffer.len();
            buffer.edit([(entire_buffer, "fn main() {}")], None, cx);
        });
        cx.executor().run_until_parked();

        // The stale response must be discarded rather than indexing the rebuilt, much
        // shorter chunk cache with the pending request's chunk id.
        unblock_request.send(()).unwrap();
        for (chunk_range, hint_task) in hint_tasks {
            let hints = hint_task
                .await
                .expect("过期的内联提示响应不应导致获取失败");
            assert!(
                hints.is_empty(),
                "块 {chunk_range:?} 的过期响应不应产生提示,实际得到 {hints:?}"
            );
        }
    }

    #[gpui::test(iterations = 4)]
    async fn test_large_buffer_inlay_requests_split(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                ..InlayHintSettingsContent::default()
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": format!("fn main() {{\n{}\n}}", "let i = 5;\n".repeat(500)),
                "other.rs": "// Test file",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;

        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());

        let lsp_request_ranges = Arc::new(Mutex::new(Vec::new()));
        let lsp_request_count = Arc::new(AtomicUsize::new(0));
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new({
                    let lsp_request_ranges = lsp_request_ranges.clone();
                    let lsp_request_count = lsp_request_count.clone();
                    move |fake_server| {
                        let closure_lsp_request_ranges = Arc::clone(&lsp_request_ranges);
                        let closure_lsp_request_count = Arc::clone(&lsp_request_count);
                        fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                            move |params, _| {
                                let task_lsp_request_ranges =
                                    Arc::clone(&closure_lsp_request_ranges);
                                let task_lsp_request_count = Arc::clone(&closure_lsp_request_count);
                                async move {
                                    assert_eq!(
                                        params.text_document.uri,
                                        lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap(),
                                    );

                                    task_lsp_request_ranges.lock().push(params.range);
                                    task_lsp_request_count.fetch_add(1, Ordering::Release);
                                    Ok(Some(vec![lsp::InlayHint {
                                        position: params.range.start,
                                        label: lsp::InlayHintLabel::String(
                                            params.range.end.line.to_string(),
                                        ),
                                        kind: None,
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    }]))
                                }
                            },
                        );
                    }
                })),
                ..FakeLspAdapter::default()
            },
        );

        let buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let editor =
            cx.add_window(|window, cx| Editor::for_buffer(buffer, Some(project), window, cx));
        cx.executor().run_until_parked();
        let _fake_server = fake_servers.next().await.unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        let ranges = lsp_request_ranges
            .lock()
            .drain(..)
            .sorted_by_key(|r| r.start)
            .collect::<Vec<_>>();
        assert_eq!(
            ranges.len(),
            1,
            "最初应查询 1 个范围,但实际得到:{ranges:?}"
        );

        editor
            .update(cx, |editor, window, cx| {
                editor.scroll_screen(&ScrollAmount::Page(1.0), window, cx);
            })
            .unwrap();
        // Wait for the first hints request to fire off
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, window, cx| {
                editor.scroll_screen(&ScrollAmount::Page(1.0), window, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();
        let visible_range_after_scrolls = editor_visible_range(&editor, cx);
        let visible_line_count = editor
            .update(cx, |editor, _window, _| {
                editor.visible_line_count().unwrap()
            })
            .unwrap();
        let selection_in_cached_range = editor
            .update(cx, |editor, _window, cx| {
                let ranges = lsp_request_ranges
                    .lock()
                    .drain(..)
                    .sorted_by_key(|r| r.start)
                    .collect::<Vec<_>>();
                assert_eq!(
                    ranges.len(),
                    2,
                    "两次滚动后应查询 2 个范围,但实际得到:{ranges:?}"
                );
                let first_scroll = &ranges[0];
                let second_scroll = &ranges[1];
                assert_eq!(
                    first_scroll.end.line, second_scroll.start.line,
                    "滚动后应查询 2 个相邻范围,但实际得到:{ranges:?}"
                );

                let lsp_requests = lsp_request_count.load(Ordering::Acquire);
                assert_eq!(
                    lsp_requests, 3,
                    "最初应查询提示,并在每次滚动后查询(共 2 次)"
                );
                assert_eq!(
                    vec!["50".to_string(), "100".to_string(), "150".to_string()],
                    cached_hint_labels(editor, cx),
                    "每次都应查询 50 行宽的块"
                );
                assert_eq!(
                    vec!["50".to_string(), "100".to_string(), "150".to_string()],
                    visible_hint_labels(editor, cx),
                    "编辑器应仅显示其滚动到的提示"
                );

                let mut selection_in_cached_range = visible_range_after_scrolls.end;
                selection_in_cached_range.row -= visible_line_count.ceil() as u32;
                selection_in_cached_range
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(
                    SelectionEffects::scroll(Autoscroll::center()),
                    window,
                    cx,
                    |s| s.select_ranges([selection_in_cached_range..selection_in_cached_range]),
                );
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();
        editor.update(cx, |_, _, _| {
            let ranges = lsp_request_ranges
                .lock()
                .drain(..)
                .sorted_by_key(|r| r.start)
                .collect::<Vec<_>>();
            assert!(ranges.is_empty(), "返回到带有缓存提示的选择后,不应再进行新的范围或 LSP 查询");
            assert_eq!(lsp_request_count.load(Ordering::Acquire), 3, "在缓存的块内进行选择时,不应发出新的请求");
        }).unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.handle_input("++++more text++++", window, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.executor().run_until_parked();
        editor.update(cx, |editor, _window, cx| {
            let mut ranges = lsp_request_ranges.lock().drain(..).collect::<Vec<_>>();
            ranges.sort_by_key(|r| r.start);

            assert_eq!(ranges.len(), 2,
                "编辑时,应滚动到选择并查询其周围的范围:该范围应分成 2 个 50 行宽的块。但实际得到的查询范围为 {ranges:?}");
            let first_chunk = &ranges[0];
            let second_chunk = &ranges[1];
            assert!(first_chunk.end.line == second_chunk.start.line,
                "第一个块 {first_chunk:?} 应在第二个块 {second_chunk:?} 之前");
            assert!(first_chunk.start.line < selection_in_cached_range.row,
                "提示应在查询范围开始后与所选范围一起查询");

            let lsp_requests = lsp_request_count.load(Ordering::Acquire);
            assert_eq!(lsp_requests, 5, "应重新查询两个块");
            assert_eq!(vec!["100".to_string(), "150".to_string()], cached_hint_labels(editor, cx),
                "编辑后应具有来自新 LSP 响应的(更少的)提示");
            assert_eq!(vec!["100".to_string(), "150".to_string()], visible_hint_labels(editor, cx), "应仅显示新缓存集中的可见提示(在中心)");
        }).unwrap();
    }

    fn editor_visible_range(
        editor: &WindowHandle<Editor>,
        cx: &mut gpui::TestAppContext,
    ) -> Range<Point> {
        let ranges = editor
            .update(cx, |editor, _window, cx| editor.visible_buffer_ranges(cx))
            .unwrap();
        assert_eq!(
            ranges.len(),
            1,
            "单个缓冲区应产生具有可见范围的单个摘录"
        );
        let (buffer_snapshot, visible_range, _) = ranges.into_iter().next().unwrap();
        visible_range.to_point(&buffer_snapshot)
    }

    #[gpui::test]
    async fn test_multiple_excerpts_large_multibuffer(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
                path!("/a"),
                json!({
                    "main.rs": format!("fn main() {{\n{}\n}}", (0..501).map(|i| format!("let i = {i};\n")).collect::<String>()),
                    "other.rs": format!("fn main() {{\n{}\n}}", (0..501).map(|j| format!("let j = {j};\n")).collect::<String>()),
                }),
            )
            .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;

        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        let language = rust_lang();
        language_registry.add(language);
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                ..FakeLspAdapter::default()
            },
        );

        let (buffer_1, _handle1) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let (buffer_2, _handle2) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/other.rs"), cx)
            })
            .await
            .unwrap();
        let multibuffer = cx.new(|cx| {
            let mut multibuffer = MultiBuffer::new(Capability::ReadWrite);
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(0),
                buffer_1.clone(),
                [
                    Point::new(0, 0)..Point::new(2, 0),
                    Point::new(4, 0)..Point::new(11, 0),
                    Point::new(22, 0)..Point::new(33, 0),
                    Point::new(44, 0)..Point::new(55, 0),
                    Point::new(56, 0)..Point::new(66, 0),
                    Point::new(67, 0)..Point::new(77, 0),
                ],
                0,
                cx,
            );
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(1),
                buffer_2.clone(),
                [
                    Point::new(0, 1)..Point::new(2, 1),
                    Point::new(4, 1)..Point::new(11, 1),
                    Point::new(22, 1)..Point::new(33, 1),
                    Point::new(44, 1)..Point::new(55, 1),
                    Point::new(56, 1)..Point::new(66, 1),
                    Point::new(67, 1)..Point::new(77, 1),
                ],
                0,
                cx,
            );
            multibuffer
        });

        cx.executor().run_until_parked();
        let editor = cx.add_window(|window, cx| {
            Editor::for_multibuffer(multibuffer, Some(project.clone()), window, cx)
        });

        let editor_edited = Arc::new(AtomicBool::new(false));
        let fake_server = fake_servers.next().await.unwrap();
        let closure_editor_edited = Arc::clone(&editor_edited);
        fake_server
            .set_request_handler::<lsp::request::InlayHintRequest, _, _>(move |params, _| {
                let task_editor_edited = Arc::clone(&closure_editor_edited);
                async move {
                    let hint_text = if params.text_document.uri
                        == lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap()
                    {
                        "主要提示"
                    } else if params.text_document.uri
                        == lsp::Uri::from_file_path(path!("/a/other.rs")).unwrap()
                    {
                        "其他提示"
                    } else {
                        panic!("意外的 URI:{:?}", params.text_document.uri);
                    };

                    // one hint per excerpt
                    let positions = [
                        lsp::Position::new(0, 2),
                        lsp::Position::new(4, 2),
                        lsp::Position::new(22, 2),
                        lsp::Position::new(44, 2),
                        lsp::Position::new(56, 2),
                        lsp::Position::new(67, 2),
                    ];
                    let out_of_range_hint = lsp::InlayHint {
                        position: lsp::Position::new(
                            params.range.start.line + 99,
                            params.range.start.character + 99,
                        ),
                        label: lsp::InlayHintLabel::String(
                            "超出摘录范围,应被忽略".to_string(),
                        ),
                        kind: None,
                        text_edits: None,
                        tooltip: None,
                        padding_left: None,
                        padding_right: None,
                        data: None,
                    };

                    let edited = task_editor_edited.load(Ordering::Acquire);
                    Ok(Some(
                        std::iter::once(out_of_range_hint)
                            .chain(positions.into_iter().enumerate().map(|(i, position)| {
                                lsp::InlayHint {
                                    position,
                                    label: lsp::InlayHintLabel::String(format!(
                                        "{hint_text}{E} #{i}",
                                        E = if edited { "(已编辑)" } else { "" },
                                    )),
                                    kind: None,
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: None,
                                    data: None,
                                }
                            }))
                            .collect(),
                    ))
                }
            })
            .next()
            .await;
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec![
                    "主要提示 #0".to_string(),
                    "主要提示 #1".to_string(),
                    "主要提示 #2".to_string(),
                    "主要提示 #3".to_string(),
                    "主要提示 #4".to_string(),
                    "主要提示 #5".to_string(),
                ];
                assert_eq!(
                    expected_hints,
                    sorted_cached_hint_labels(editor, cx),
                    "当滚动到多缓冲区边缘时,应仅查询其可见摘录的内联提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(
                    SelectionEffects::scroll(Autoscroll::Next),
                    window,
                    cx,
                    |s| s.select_ranges([Point::new(4, 0)..Point::new(4, 0)]),
                );
                editor.change_selections(
                    SelectionEffects::scroll(Autoscroll::Next),
                    window,
                    cx,
                    |s| s.select_ranges([Point::new(22, 0)..Point::new(22, 0)]),
                );
                editor.change_selections(
                    SelectionEffects::scroll(Autoscroll::Next),
                    window,
                    cx,
                    |s| s.select_ranges([Point::new(57, 0)..Point::new(57, 0)]),
                );
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec![
                    "主要提示 #0".to_string(),
                    "主要提示 #1".to_string(),
                    "主要提示 #2".to_string(),
                    "主要提示 #3".to_string(),
                    "主要提示 #4".to_string(),
                    "主要提示 #5".to_string(),
                ];
                assert_eq!(expected_hints, sorted_cached_hint_labels(editor, cx),
                    "滚动后不会立即显示新提示,我们需要等待缓冲区注册");
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec![
                    "主要提示 #0".to_string(),
                    "主要提示 #1".to_string(),
                    "主要提示 #2".to_string(),
                    "主要提示 #3".to_string(),
                    "主要提示 #4".to_string(),
                    "主要提示 #5".to_string(),
                    "其他提示 #0".to_string(),
                    "其他提示 #1".to_string(),
                    "其他提示 #2".to_string(),
                    "其他提示 #3".to_string(),
                ];
                assert_eq!(
                    expected_hints,
                    sorted_cached_hint_labels(editor, cx),
                    "滚动到新缓冲区并等待其注册后,应出现新的提示");
                assert_eq!(
                    expected_hints,
                    visible_hint_labels(editor, cx),
                    "编辑器应仅显示可见提示",
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(
                    SelectionEffects::scroll(Autoscroll::Next),
                    window,
                    cx,
                    |s| s.select_ranges([Point::new(100, 0)..Point::new(100, 0)]),
                );
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec![
                    "主要提示 #0".to_string(),
                    "主要提示 #1".to_string(),
                    "主要提示 #2".to_string(),
                    "主要提示 #3".to_string(),
                    "主要提示 #4".to_string(),
                    "主要提示 #5".to_string(),
                    "其他提示 #0".to_string(),
                    "其他提示 #1".to_string(),
                    "其他提示 #2".to_string(),
                    "其他提示 #3".to_string(),
                    "其他提示 #4".to_string(),
                    "其他提示 #5".to_string(),
                ];
                assert_eq!(
                    expected_hints,
                    sorted_cached_hint_labels(editor, cx),
                    "多缓冲区滚动到末尾后,应获取所有摘录的所有提示"
                );
                assert_eq!(
                    expected_hints,
                    visible_hint_labels(editor, cx),
                    "编辑器仅显示滚动时可见的摘录的提示"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(
                    SelectionEffects::scroll(Autoscroll::Next),
                    window,
                    cx,
                    |s| s.select_ranges([Point::new(4, 0)..Point::new(4, 0)]),
                );
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec![
                    "主要提示 #0".to_string(),
                    "主要提示 #1".to_string(),
                    "主要提示 #2".to_string(),
                    "主要提示 #3".to_string(),
                    "主要提示 #4".to_string(),
                    "主要提示 #5".to_string(),
                    "其他提示 #0".to_string(),
                    "其他提示 #1".to_string(),
                    "其他提示 #2".to_string(),
                    "其他提示 #3".to_string(),
                    "其他提示 #4".to_string(),
                    "其他提示 #5".to_string(),
                ];
                assert_eq!(
                    expected_hints,
                    sorted_cached_hint_labels(editor, cx),
                    "多缓冲区滚动到末尾后,进一步向上滚动不应带来更多提示"
                );
                assert_eq!(
                    expected_hints,
                    visible_hint_labels(editor, cx),
                );
            })
            .unwrap();

        // We prepare to change the scrolling on edit, but do not scroll yet
        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                    s.select_ranges([Point::new(57, 0)..Point::new(57, 0)])
                });
            })
            .unwrap();
        cx.executor().run_until_parked();
        // Edit triggers the scrolling too
        editor_edited.store(true, Ordering::Release);
        editor
            .update(cx, |editor, window, cx| {
                editor.handle_input("++++more text++++", window, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        // Wait again to trigger the inlay hints fetch on scroll
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _window, cx| {
                let expected_hints = vec![
                    "主要提示(已编辑) #0".to_string(),
                    "主要提示(已编辑) #1".to_string(),
                    "主要提示(已编辑) #2".to_string(),
                    "主要提示(已编辑) #3".to_string(),
                    "主要提示(已编辑) #4".to_string(),
                    "主要提示(已编辑) #5".to_string(),
                    "其他提示(已编辑) #0".to_string(),
                    "其他提示(已编辑) #1".to_string(),
                    "其他提示(已编辑) #2".to_string(),
                    "其他提示(已编辑) #3".to_string(),
                ];
                assert_eq!(
                    expected_hints,
                    sorted_cached_hint_labels(editor, cx),
                    "After multibuffer edit, editor gets scrolled back to the last selection; \
                all hints should be invalidated and required for all of its visible excerpts"
                );
                assert_eq!(
                    expected_hints,
                    visible_hint_labels(editor, cx),
                    "所有摘录都应获得其提示"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_editing_in_multi_buffer(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                ..InlayHintSettingsContent::default()
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": format!("fn main() {{\n{}\n}}", (0..200).map(|i| format!("let i = {i};\n")).collect::<String>()),
                "lib.rs": r#"let a = 1;
let b = 2;
let c = 3;"#
            }),
        )
        .await;

        let lsp_request_ranges = Arc::new(Mutex::new(Vec::new()));

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;
        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        let language = rust_lang();
        language_registry.add(language);

        let closure_ranges_fetched = lsp_request_ranges.clone();
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new(move |fake_server| {
                    let closure_ranges_fetched = closure_ranges_fetched.clone();
                    fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                        move |params, _| {
                            let closure_ranges_fetched = closure_ranges_fetched.clone();
                            async move {
                                let prefix = if params.text_document.uri
                                    == lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap()
                                {
                                    closure_ranges_fetched
                                        .lock()
                                        .push(("main.rs", params.range));
                                    "main.rs"
                                } else if params.text_document.uri
                                    == lsp::Uri::from_file_path(path!("/a/lib.rs")).unwrap()
                                {
                                    closure_ranges_fetched.lock().push(("lib.rs", params.range));
                                    "lib.rs"
                                } else {
                                    panic!("意外的文件路径 {:?}", params.text_document.uri);
                                };
                                Ok(Some(
                                    (params.range.start.line..params.range.end.line)
                                        .map(|row| lsp::InlayHint {
                                            position: lsp::Position::new(row, 0),
                                            label: lsp::InlayHintLabel::String(format!(
                                                "{prefix} 内联提示 #{row}"
                                            )),
                                            kind: Some(lsp::InlayHintKind::TYPE),
                                            text_edits: None,
                                            tooltip: None,
                                            padding_left: None,
                                            padding_right: None,
                                            data: None,
                                        })
                                        .collect(),
                                ))
                            }
                        },
                    );
                })),
                ..FakeLspAdapter::default()
            },
        );

        let (buffer_1, _handle_1) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let (buffer_2, _handle_2) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/lib.rs"), cx)
            })
            .await
            .unwrap();
        let multi_buffer = cx.new(|cx| {
            let mut multibuffer = MultiBuffer::new(Capability::ReadWrite);
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(0),
                buffer_1.clone(),
                [
                    Point::new(49, 0)..Point::new(53, 0),
                    Point::new(70, 0)..Point::new(73, 0),
                ],
                0,
                cx,
            );
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(1),
                buffer_2.clone(),
                [Point::new(0, 0)..Point::new(4, 0)],
                0,
                cx,
            );
            multibuffer
        });

        let editor = cx.add_window(|window, cx| {
            let mut editor =
                Editor::for_multibuffer(multi_buffer, Some(project.clone()), window, cx);
            editor.change_selections(SelectionEffects::default(), window, cx, |s| {
                s.select_ranges([MultiBufferOffset(0)..MultiBufferOffset(0)])
            });
            editor
        });

        let _fake_server = fake_servers.next().await.unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        assert_eq!(
            vec![
                (
                    "lib.rs",
                    lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(2, 10))
                ),
                (
                    "main.rs",
                    lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(50, 0))
                ),
                (
                    "main.rs",
                    lsp::Range::new(lsp::Position::new(50, 0), lsp::Position::new(100, 0))
                ),
            ],
            lsp_request_ranges
                .lock()
                .drain(..)
                .sorted_by_key(|(prefix, r)| (prefix.to_owned(), r.start))
                .collect::<Vec<_>>(),
            "对于大型缓冲区,应查询覆盖两个可见摘录的块"
        );
        editor
            .update(cx, |editor, _window, cx| {
                assert_eq!(
                    (0..2)
                        .map(|i| format!("lib.rs 内联提示 #{i}"))
                        .chain((0..100).map(|i| format!("main.rs 内联提示 #{i}")))
                        .collect::<Vec<_>>(),
                    sorted_cached_hint_labels(editor, cx),
                    "两个块都应提供其内联提示"
                );
                assert_eq!(
                    vec![
                        "main.rs 内联提示 #49".to_owned(),
                        "main.rs 内联提示 #50".to_owned(),
                        "main.rs 内联提示 #51".to_owned(),
                        "main.rs 内联提示 #52".to_owned(),
                        "main.rs 内联提示 #53".to_owned(),
                        "main.rs 内联提示 #70".to_owned(),
                        "main.rs 内联提示 #71".to_owned(),
                        "main.rs 内联提示 #72".to_owned(),
                        "main.rs 内联提示 #73".to_owned(),
                        "lib.rs 内联提示 #0".to_owned(),
                        "lib.rs Inlay hint #1".to_owned(),
                    ],
                    visible_hint_labels(editor, cx),
                    "只有可见摘录中的提示才应添加到编辑器中"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.handle_input("a", window, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(1000));
        cx.executor().run_until_parked();
        assert_eq!(
            vec![
                (
                    "lib.rs",
                    lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(2, 10))
                ),
                (
                    "main.rs",
                    lsp::Range::new(lsp::Position::new(0, 0), lsp::Position::new(50, 0))
                ),
                (
                    "main.rs",
                    lsp::Range::new(lsp::Position::new(50, 0), lsp::Position::new(100, 0))
                ),
            ],
            lsp_request_ranges
                .lock()
                .drain(..)
                .sorted_by_key(|(prefix, r)| (prefix.to_owned(), r.start))
                .collect::<Vec<_>>(),
            "编辑时应重新查询相同的代码块"
        );
        editor
            .update(cx, |editor, _window, cx| {
                assert_eq!(
                    (0..2)
                        .map(|i| format!("lib.rs 内联提示 #{i}"))
                        .chain((0..100).map(|i| format!("main.rs 内联提示 #{i}")))
                        .collect::<Vec<_>>(),
                    sorted_cached_hint_labels(editor, cx),
                    "编辑后应重新插入相同的提示"
                );
                assert_eq!(
                    vec![
                        "main.rs 内联提示 #49".to_owned(),
                        "main.rs 内联提示 #50".to_owned(),
                        "main.rs 内联提示 #51".to_owned(),
                        "main.rs 内联提示 #52".to_owned(),
                        "main.rs 内联提示 #53".to_owned(),
                        "main.rs 内联提示 #70".to_owned(),
                        "main.rs 内联提示 #71".to_owned(),
                        "main.rs 内联提示 #72".to_owned(),
                        "main.rs 内联提示 #73".to_owned(),
                        "lib.rs 内联提示 #0".to_owned(),
                        "lib.rs Inlay hint #1".to_owned(),
                    ],
                    visible_hint_labels(editor, cx),
                    "编辑后应将相同的提示重新插入到编辑器中"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_excerpts_removed(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(false),
                show_parameter_hints: Some(false),
                show_other_hints: Some(false),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": format!("fn main() {{\n{}\n}}", (0..501).map(|i| format!("let i = {i};\n")).collect::<String>()),
                "other.rs": format!("fn main() {{\n{}\n}}", (0..501).map(|j| format!("let j = {j};\n")).collect::<String>()),
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;

        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                ..FakeLspAdapter::default()
            },
        );

        let (buffer_1, _handle) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let (buffer_2, _handle2) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/other.rs"), cx)
            })
            .await
            .unwrap();
        let multibuffer = cx.new(|_| MultiBuffer::new(Capability::ReadWrite));
        multibuffer.update(cx, |multibuffer, cx| {
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(0),
                buffer_1.clone(),
                [Point::new(0, 0)..Point::new(2, 0)],
                0,
                cx,
            );
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(1),
                buffer_2.clone(),
                [Point::new(0, 1)..Point::new(2, 1)],
                0,
                cx,
            );
        });

        cx.executor().run_until_parked();
        let editor = cx.add_window(|window, cx| {
            Editor::for_multibuffer(multibuffer, Some(project.clone()), window, cx)
        });
        let editor_edited = Arc::new(AtomicBool::new(false));
        let fake_server = fake_servers.next().await.unwrap();
        let closure_editor_edited = Arc::clone(&editor_edited);
        fake_server
            .set_request_handler::<lsp::request::InlayHintRequest, _, _>(move |params, _| {
                let task_editor_edited = Arc::clone(&closure_editor_edited);
                async move {
                    let hint_text = if params.text_document.uri
                        == lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap()
                    {
                        "主要提示"
                    } else if params.text_document.uri
                        == lsp::Uri::from_file_path(path!("/a/other.rs")).unwrap()
                    {
                        "其他提示"
                    } else {
                        panic!("意外的 URI:{:?}", params.text_document.uri);
                    };

                    let positions = [
                        lsp::Position::new(0, 2),
                        lsp::Position::new(4, 2),
                        lsp::Position::new(22, 2),
                        lsp::Position::new(44, 2),
                        lsp::Position::new(56, 2),
                        lsp::Position::new(67, 2),
                    ];
                    let out_of_range_hint = lsp::InlayHint {
                        position: lsp::Position::new(
                            params.range.start.line + 99,
                            params.range.start.character + 99,
                        ),
                        label: lsp::InlayHintLabel::String(
                            "超出摘录范围,应被忽略".to_string(),
                        ),
                        kind: None,
                        text_edits: None,
                        tooltip: None,
                        padding_left: None,
                        padding_right: None,
                        data: None,
                    };

                    let edited = task_editor_edited.load(Ordering::Acquire);
                    Ok(Some(
                        std::iter::once(out_of_range_hint)
                            .chain(positions.into_iter().enumerate().map(|(i, position)| {
                                lsp::InlayHint {
                                    position,
                                    label: lsp::InlayHintLabel::String(format!(
                                        "{hint_text}{} #{i}",
                                        if edited { "(已编辑)" } else { "" },
                                    )),
                                    kind: None,
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: None,
                                    data: None,
                                }
                            }))
                            .collect(),
                    ))
                }
            })
            .next()
            .await;
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec![
                        "主要提示 #0".to_string(),
                        "主要提示 #1".to_string(),
                        "主要提示 #2".to_string(),
                        "主要提示 #3".to_string(),
                        "其他提示 #0".to_string(),
                        "其他提示 #1".to_string(),
                        "其他提示 #2".to_string(),
                        "其他提示 #3".to_string(),
                    ],
                    sorted_cached_hint_labels(editor, cx),
                    "尽管提示显示已禁用,缓存仍应更新两个摘录;选择第二个缓冲区后,它现在已注册到语言服务器,应获取其提示"
                );
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "所有提示均已禁用,即使缓存中存在也不应显示"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, _, cx| {
                editor.buffer().update(cx, |multibuffer, cx| {
                    multibuffer.remove_excerpts(PathKey::sorted(1), cx);
                })
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec![
                        "主要提示 #0".to_string(),
                        "主要提示 #1".to_string(),
                        "主要提示 #2".to_string(),
                        "主要提示 #3".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                    "对于已移除的摘录,应清理相应的缓存提示,因为其缓冲区已被丢弃"
                );
                assert!(
                visible_hint_labels(editor, cx).is_empty(),
                "所有提示均已禁用,即使缓存中存在也不应显示"
            );
            })
            .unwrap();

        update_test_language_settings(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec![
                        "主要提示 #0".to_string(),
                        "主要提示 #1".to_string(),
                        "主要提示 #2".to_string(),
                        "主要提示 #3".to_string(),
                    ],
                    cached_hint_labels(editor, cx),
                    "提示显示设置更改不应更改缓存"
                );
                assert_eq!(
                    vec![
                        "主要提示 #0".to_string(),
                    ],
                    visible_hint_labels(editor, cx),
                    "设置更改应使缓存提示可见,但仅显示剩余摘录中可见的提示"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_inside_char_boundary_range_hints(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": format!(r#"fn main() {{\n{}\n}}"#, format!("let i = {};\n", "√".repeat(10)).repeat(500)),
                "other.rs": "// Test file",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;

        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());
        language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new(move |fake_server| {
                    let lsp_request_count = Arc::new(AtomicU32::new(0));
                    fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                        move |params, _| {
                            let i = lsp_request_count.fetch_add(1, Ordering::Release) + 1;
                            async move {
                                assert_eq!(
                                    params.text_document.uri,
                                    lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap(),
                                );
                                let query_start = params.range.start;
                                Ok(Some(vec![lsp::InlayHint {
                                    position: query_start,
                                    label: lsp::InlayHintLabel::String(i.to_string()),
                                    kind: None,
                                    text_edits: None,
                                    tooltip: None,
                                    padding_left: None,
                                    padding_right: None,
                                    data: None,
                                }]))
                            }
                        },
                    );
                })),
                ..FakeLspAdapter::default()
            },
        );

        let buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let editor =
            cx.add_window(|window, cx| Editor::for_buffer(buffer, Some(project), window, cx));

        // Allow LSP to initialize
        cx.executor().run_until_parked();

        // Establish a viewport and explicitly trigger hint refresh.
        // This ensures we control exactly when hints are requested.
        editor
            .update(cx, |editor, window, cx| {
                editor.set_visible_line_count(50.0, window, cx);
                editor.set_visible_column_count(120.0);
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();

        // Allow LSP initialization and hint request/response to complete.
        // Use multiple advance_clock + run_until_parked cycles to ensure all async work completes.
        for _ in 0..5 {
            cx.executor().advance_clock(Duration::from_millis(100));
            cx.executor().run_until_parked();
        }

        // At this point we should have exactly one hint from our explicit refresh.
        // The test verifies that hints at character boundaries are handled correctly.
        editor
            .update(cx, |editor, _, cx| {
                assert!(
                    !cached_hint_labels(editor, cx).is_empty(),
                    "刷新后应至少有一个提示"
                );
                assert!(
                    !visible_hint_labels(editor, cx).is_empty(),
                    "应至少有一个可见提示"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_toggle_inlay_hints(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(false),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let (_, editor, _fake_server) = prepare_test_objects(cx, |fake_server, file_with_hints| {
            let lsp_request_count = Arc::new(AtomicU32::new(0));
            fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                move |params, _| {
                    let lsp_request_count = lsp_request_count.clone();
                    async move {
                        assert_eq!(
                            params.text_document.uri,
                            lsp::Uri::from_file_path(file_with_hints).unwrap(),
                        );

                        let i = lsp_request_count.fetch_add(1, Ordering::AcqRel) + 1;
                        Ok(Some(vec![lsp::InlayHint {
                            position: lsp::Position::new(0, i),
                            label: lsp::InlayHintLabel::String(i.to_string()),
                            kind: None,
                            text_edits: None,
                            tooltip: None,
                            padding_left: None,
                            padding_right: None,
                            data: None,
                        }]))
                    }
                },
            );
        })
        .await;

        editor
            .update(cx, |editor, window, cx| {
                editor.toggle_inlay_hints(&crate::ToggleInlayHints, window, cx)
            })
            .unwrap();

        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "尽管在设置中已禁用,切换后仍应显示内联提示"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.toggle_inlay_hints(&crate::ToggleInlayHints, window, cx)
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "编辑器中的切换操作不会更改缓存"
                );
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "第二次切换后应清除提示"
                );
            })
            .unwrap();

        update_test_language_settings(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                let expected_hints = vec!["1".to_string()];
                assert_eq!(
                    expected_hints,
                    cached_hint_labels(editor, cx),
                    "在设置中启用提示后不应查询 LSP 提示,因为文件版本相同"
                );
                assert_eq!(expected_hints, visible_hint_labels(editor, cx));
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.toggle_inlay_hints(&crate::ToggleInlayHints, window, cx)
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "编辑器中的切换操作不会更改缓存"
                );
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "在设置中启用并进行第三次切换后应清除提示"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.toggle_inlay_hints(&crate::ToggleInlayHints, window, cx)
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor.update(cx, |editor, _, cx| {
            let expected_hints = vec!["1".to_string()];
            assert_eq!(
                expected_hints,
                cached_hint_labels(editor,cx),
                "在设置中启用提示并重新打开后不应查询 LSP 提示"
            );
            assert_eq!(expected_hints, visible_hint_labels(editor, cx));
        }).unwrap();
    }

    #[gpui::test]
    async fn test_modifiers_change(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let (_, editor, _fake_server) = prepare_test_objects(cx, |fake_server, file_with_hints| {
            let lsp_request_count = Arc::new(AtomicU32::new(0));
            fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                move |params, _| {
                    let lsp_request_count = lsp_request_count.clone();
                    async move {
                        assert_eq!(
                            params.text_document.uri,
                            lsp::Uri::from_file_path(file_with_hints).unwrap(),
                        );

                        let i = lsp_request_count.fetch_add(1, Ordering::AcqRel) + 1;
                        Ok(Some(vec![lsp::InlayHint {
                            position: lsp::Position::new(0, i),
                            label: lsp::InlayHintLabel::String(i.to_string()),
                            kind: None,
                            text_edits: None,
                            tooltip: None,
                            padding_left: None,
                            padding_right: None,
                            data: None,
                        }]))
                    }
                },
            );
        })
        .await;

        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "尽管在设置中已禁用,切换后仍应显示内联提示"
                );
                assert_eq!(vec!["1".to_string()], visible_hint_labels(editor, cx));
            })
            .unwrap();

        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(true), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "修饰键更改时缓存不会发生任何变化"
                );
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "当修饰键更改且提示已打开时,应隐藏编辑器内联提示"
                );
            })
            .unwrap();
        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(true), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(vec!["1".to_string()], cached_hint_labels(editor, cx));
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "后续相同类型的修饰键更改不会发生任何变化"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(false), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "当修饰键更改关闭时,不会发送额外请求"
                );
                assert_eq!(
                    vec!["1".to_string()],
                    visible_hint_labels(editor, cx),
                    "当修饰键更改关闭时,提示会重新显示在编辑器中"
                );
            })
            .unwrap();
        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(false), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(vec!["1".to_string()], cached_hint_labels(editor, cx));
                assert_eq!(
                    vec!["1".to_string()],
                    visible_hint_labels(editor, cx),
                    "后续相同类型的修饰键更改不会发生任何变化(2)"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.toggle_inlay_hints(&crate::ToggleInlayHints, window, cx)
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "修饰键更改时缓存不会发生任何变化"
                );
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "关闭时,应隐藏编辑器内联提示"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(true), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "修饰键更改时缓存不会发生任何变化"
                );
                assert_eq!(
                    vec!["1".to_string()],
                    visible_hint_labels(editor, cx),
                    "当修饰键更改且提示已关闭时,应显示编辑器内联提示"
                );
            })
            .unwrap();
        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(true), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(vec!["1".to_string()], cached_hint_labels(editor, cx));
                assert_eq!(
                    vec!["1".to_string()],
                    visible_hint_labels(editor, cx),
                    "后续相同类型的修饰键更改不会发生任何变化"
                );
            })
            .unwrap();

        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(false), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(
                    vec!["1".to_string()],
                    cached_hint_labels(editor, cx),
                    "当修饰键更改关闭时,不会发送额外请求"
                );
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "当修饰键更改关闭时,编辑器提示恢复到其关闭状态"
                );
            })
            .unwrap();
        editor
            .update(cx, |editor, _, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::ModifiersChanged(false), cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        editor
            .update(cx, |editor, _, cx| {
                assert_eq!(vec!["1".to_string()], cached_hint_labels(editor, cx));
                assert_eq!(
                    Vec::<String>::new(),
                    visible_hint_labels(editor, cx),
                    "后续相同类型的修饰键更改不会发生任何变化(3)"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_inlays_at_the_same_place(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": "fn main() {
                    let x = 42;
                    std::thread::scope(|s| {
                        s.spawn(|| {
                            let _x = x;
                        });
                    });
                }",
                "other.rs": "// Test file",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;

        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());
        language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..Default::default()
                },
                initializer: Some(Box::new(move |fake_server| {
                    fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                        move |params, _| async move {
                            assert_eq!(
                                params.text_document.uri,
                                lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap(),
                            );
                            Ok(Some(
                                serde_json::from_value(json!([
                                    {
                                        "position": {
                                            "line": 3,
                                            "character": 16
                                        },
                                        "label": "move",
                                        "paddingLeft": false,
                                        "paddingRight": false
                                    },
                                    {
                                        "position": {
                                            "line": 3,
                                            "character": 16
                                        },
                                        "label": "(",
                                        "paddingLeft": false,
                                        "paddingRight": false
                                    },
                                    {
                                        "position": {
                                            "line": 3,
                                            "character": 16
                                        },
                                        "label": [
                                            {
                                                "value": "&x"
                                            }
                                        ],
                                        "paddingLeft": false,
                                        "paddingRight": false,
                                        "data": {
                                            "file_id": 0
                                        }
                                    },
                                    {
                                        "position": {
                                            "line": 3,
                                            "character": 16
                                        },
                                        "label": ")",
                                        "paddingLeft": false,
                                        "paddingRight": true
                                    },
                                    // not a correct syntax, but checks that same symbols at the same place
                                    // are not deduplicated
                                    {
                                        "position": {
                                            "line": 3,
                                            "character": 16
                                        },
                                        "label": ")",
                                        "paddingLeft": false,
                                        "paddingRight": true
                                    },
                                ]))
                                .unwrap(),
                            ))
                        },
                    );
                })),
                ..FakeLspAdapter::default()
            },
        );

        let buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();

        // Use a VisualTestContext and explicitly establish a viewport on the editor (the production
        // trigger for `NewLinesShown` / inlay hint refresh) by setting visible line/column counts.
        let (editor_entity, cx) =
            cx.add_window_view(|window, cx| Editor::for_buffer(buffer, Some(project), window, cx));

        editor_entity.update_in(cx, |editor, window, cx| {
            // Establish a viewport. The exact values are not important for this test; we just need
            // the editor to consider itself visible so the refresh pipeline runs.
            editor.set_visible_line_count(50.0, window, cx);
            editor.set_visible_column_count(120.0);

            // Explicitly trigger a refresh now that the viewport exists.
            editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
        });
        cx.executor().run_until_parked();

        editor_entity.update_in(cx, |editor, window, cx| {
            editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                s.select_ranges([Point::new(10, 0)..Point::new(10, 0)])
            });
        });
        cx.executor().run_until_parked();

        // Allow any async inlay hint request/response work to complete.
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        editor_entity.update(cx, |editor, cx| {
            let expected_hints = vec![
                "move".to_string(),
                "(".to_string(),
                "&x".to_string(),
                ") ".to_string(),
                ") ".to_string(),
            ];
            assert_eq!(
                expected_hints,
                cached_hint_labels(editor, cx),
                "编辑器内联提示在放置在同一位置时应重复服务器的顺序"
            );
            assert_eq!(expected_hints, visible_hint_labels(editor, cx));
        });
    }

    #[gpui::test]
    async fn test_invalidation_and_addition_race(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                ..InlayHintSettingsContent::default()
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": r#"fn main() {
                    let x = 1;
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    ////
                    let x = "2";
                }
"#,
                "lib.rs": r#"fn aaa() {
                    let aa = 22;
                }
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //
                //

                fn bb() {
                    let bb = 33;
                }
"#
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;
        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        let language = rust_lang();
        language_registry.add(language);

        let requests_count = Arc::new(AtomicUsize::new(0));
        let closure_requests_count = requests_count.clone();
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                name: "rust-analyzer",
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new(move |fake_server| {
                    let requests_count = closure_requests_count.clone();
                    fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                        move |params, _| {
                            let requests_count = requests_count.clone();
                            async move {
                                requests_count.fetch_add(1, Ordering::Release);
                                if params.text_document.uri
                                    == lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap()
                                {
                                    Ok(Some(vec![
                                        lsp::InlayHint {
                                            position: lsp::Position::new(1, 9),
                                            label: lsp::InlayHintLabel::String(": i32".to_owned()),
                                            kind: Some(lsp::InlayHintKind::TYPE),
                                            text_edits: None,
                                            tooltip: None,
                                            padding_left: None,
                                            padding_right: None,
                                            data: None,
                                        },
                                        lsp::InlayHint {
                                            position: lsp::Position::new(19, 9),
                                            label: lsp::InlayHintLabel::String(": i33".to_owned()),
                                            kind: Some(lsp::InlayHintKind::TYPE),
                                            text_edits: None,
                                            tooltip: None,
                                            padding_left: None,
                                            padding_right: None,
                                            data: None,
                                        },
                                    ]))
                                } else if params.text_document.uri
                                    == lsp::Uri::from_file_path(path!("/a/lib.rs")).unwrap()
                                {
                                    Ok(Some(vec![
                                        lsp::InlayHint {
                                            position: lsp::Position::new(1, 10),
                                            label: lsp::InlayHintLabel::String(": i34".to_owned()),
                                            kind: Some(lsp::InlayHintKind::TYPE),
                                            text_edits: None,
                                            tooltip: None,
                                            padding_left: None,
                                            padding_right: None,
                                            data: None,
                                        },
                                        lsp::InlayHint {
                                            position: lsp::Position::new(29, 10),
                                            label: lsp::InlayHintLabel::String(": i35".to_owned()),
                                            kind: Some(lsp::InlayHintKind::TYPE),
                                            text_edits: None,
                                            tooltip: None,
                                            padding_left: None,
                                            padding_right: None,
                                            data: None,
                                        },
                                    ]))
                                } else {
                                    panic!("意外的文件路径 {:?}", params.text_document.uri);
                                }
                            }
                        },
                    );
                })),
                ..FakeLspAdapter::default()
            },
        );

        // Add another server that does send the same, duplicate hints back
        let mut fake_servers_2 = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                name: "CrabLang-ls",
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new(move |fake_server| {
                    fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                        move |params, _| async move {
                            if params.text_document.uri
                                == lsp::Uri::from_file_path(path!("/a/main.rs")).unwrap()
                            {
                                Ok(Some(vec![
                                    lsp::InlayHint {
                                        position: lsp::Position::new(1, 9),
                                        label: lsp::InlayHintLabel::String(": i32".to_owned()),
                                        kind: Some(lsp::InlayHintKind::TYPE),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    },
                                    lsp::InlayHint {
                                        position: lsp::Position::new(19, 9),
                                        label: lsp::InlayHintLabel::String(": i33".to_owned()),
                                        kind: Some(lsp::InlayHintKind::TYPE),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    },
                                ]))
                            } else if params.text_document.uri
                                == lsp::Uri::from_file_path(path!("/a/lib.rs")).unwrap()
                            {
                                Ok(Some(vec![
                                    lsp::InlayHint {
                                        position: lsp::Position::new(1, 10),
                                        label: lsp::InlayHintLabel::String(": i34".to_owned()),
                                        kind: Some(lsp::InlayHintKind::TYPE),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    },
                                    lsp::InlayHint {
                                        position: lsp::Position::new(29, 10),
                                        label: lsp::InlayHintLabel::String(": i35".to_owned()),
                                        kind: Some(lsp::InlayHintKind::TYPE),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    },
                                ]))
                            } else {
                                panic!("意外的文件路径 {:?}", params.text_document.uri);
                            }
                        },
                    );
                })),
                ..FakeLspAdapter::default()
            },
        );

        let (buffer_1, _handle_1) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let (buffer_2, _handle_2) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/lib.rs"), cx)
            })
            .await
            .unwrap();
        let multi_buffer = cx.new(|cx| {
            let mut multibuffer = MultiBuffer::new(Capability::ReadWrite);
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(0),
                buffer_2.clone(),
                [
                    Point::new(0, 0)..Point::new(10, 0),
                    Point::new(23, 0)..Point::new(34, 0),
                ],
                0,
                cx,
            );
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(1),
                buffer_1.clone(),
                [
                    Point::new(0, 0)..Point::new(10, 0),
                    Point::new(13, 0)..Point::new(23, 0),
                ],
                0,
                cx,
            );
            multibuffer
        });

        let editor = cx.add_window(|window, cx| {
            let mut editor =
                Editor::for_multibuffer(multi_buffer, Some(project.clone()), window, cx);
            editor.change_selections(SelectionEffects::default(), window, cx, |s| {
                s.select_ranges([Point::new(3, 3)..Point::new(3, 3)])
            });
            editor
        });

        let fake_server = fake_servers.next().await.unwrap();
        let _fake_server_2 = fake_servers_2.next().await.unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                assert_eq!(
                    vec![
                        ": i32".to_string(),
                        ": i32".to_string(),
                        ": i33".to_string(),
                        ": i33".to_string(),
                        ": i34".to_string(),
                        ": i34".to_string(),
                        ": i35".to_string(),
                        ": i35".to_string(),
                    ],
                    sorted_cached_hint_labels(editor, cx),
                    "我们从 2 个服务器收到重复提示并全部缓存"
                );
                assert_eq!(
                    vec![
                        ": i34".to_string(),
                        ": i35".to_string(),
                        ": i32".to_string(),
                        ": i33".to_string(),
                    ],
                    visible_hint_labels(editor, cx),
                    "lib.rs 在 main.rs 之前添加,因此其摘录应首先可见;提示应按标签去重"
                );
            })
            .unwrap();
        assert_eq!(
            requests_count.load(Ordering::Acquire),
            2,
            "每个文件应查询一次提示"
        );

        // Scroll all the way down so the 1st buffer is out of sight.
        // The selection is on the 1st buffer still.
        editor
            .update(cx, |editor, window, cx| {
                editor.scroll_screen(&ScrollAmount::Line(88.0), window, cx);
            })
            .unwrap();
        // Emulate a language server refresh request, coming in the background..
        let refresh_request = fake_server
            .request::<lsp::request::InlayHintRefreshRequest>((), lsp::DEFAULT_LSP_REQUEST_TIMEOUT);
        // Edit the 1st buffer while scrolled down and not seeing that.
        // The edit will auto scroll to the edit (1st buffer).
        editor
            .update(cx, |editor, window, cx| {
                editor.handle_input("a", window, cx);
            })
            .unwrap();
        // Add more racy additive hint tasks.
        editor
            .update(cx, |editor, window, cx| {
                editor.scroll_screen(&ScrollAmount::Line(0.2), window, cx);
            })
            .unwrap();

        cx.executor().advance_clock(Duration::from_millis(1000));
        cx.executor().run_until_parked();
        refresh_request.await.into_response().unwrap();
        editor
            .update(cx, |editor, _window, cx| {
                assert_eq!(
                    vec![
                        ": i32".to_string(),
                        ": i32".to_string(),
                        ": i33".to_string(),
                        ": i33".to_string(),
                        ": i34".to_string(),
                        ": i34".to_string(),
                        ": i35".to_string(),
                        ": i35".to_string(),
                    ],
                    sorted_cached_hint_labels(editor, cx),
                    "缓存中不应发生提示更改或重复",
                );
                assert_eq!(
                    vec![
                        ": i34".to_string(),
                        ": i35".to_string(),
                        ": i32".to_string(),
                        ": i33".to_string(),
                    ],
                    visible_hint_labels(editor, cx),
                    "编辑器摘录中不应发生提示更改或重复",
                );
            })
            .unwrap();
        assert_eq!(
            requests_count.load(Ordering::Acquire),
            4,
            "编辑文件一次后,每个文件应再查询一次提示"
        );
    }

    #[gpui::test]
    async fn test_edit_then_scroll_race(cx: &mut gpui::TestAppContext) {
        // Bug 1: An edit fires with a long debounce, and a scroll brings new lines
        // before that debounce elapses. The edit task's apply_fetched_hints removes
        // ALL visible hints (including the scroll-added ones) but only adds back
        // hints for its own chunks. The scroll chunk remains in hint_chunk_fetching,
        // so it is never re-queried, leaving it permanently empty.
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                edit_debounce_ms: Some(700),
                scroll_debounce_ms: Some(50),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                ..InlayHintSettingsContent::default()
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        let mut file_content = String::from("fn main() {\n");
        for i in 0..150 {
            file_content.push_str(&format!("    let v{i} = {i};\n"));
        }
        file_content.push_str("}\n");
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": file_content,
                "other.rs": "// Test file",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;
        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());

        let lsp_request_ranges = Arc::new(Mutex::new(Vec::new()));
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new({
                    let lsp_request_ranges = lsp_request_ranges.clone();
                    move |fake_server| {
                        let lsp_request_ranges = lsp_request_ranges.clone();
                        fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                            move |params, _| {
                                let lsp_request_ranges = lsp_request_ranges.clone();
                                async move {
                                    lsp_request_ranges.lock().push(params.range);
                                    let start_line = params.range.start.line;
                                    Ok(Some(vec![lsp::InlayHint {
                                        position: lsp::Position::new(start_line + 1, 9),
                                        label: lsp::InlayHintLabel::String(format!(
                                            "chunk_{start_line}"
                                        )),
                                        kind: Some(lsp::InlayHintKind::TYPE),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    }]))
                                }
                            },
                        );
                    }
                })),
                ..FakeLspAdapter::default()
            },
        );

        let buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let editor =
            cx.add_window(|window, cx| Editor::for_buffer(buffer, Some(project), window, cx));
        cx.executor().run_until_parked();
        let _fake_server = fake_servers.next().await.unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.set_visible_line_count(50.0, window, cx);
                editor.set_visible_column_count(120.0);
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                let visible = visible_hint_labels(editor, cx);
                assert!(
                    visible.iter().any(|h| h.starts_with("chunk_0")),
                    "最初应有 chunk_0 提示,实际为:{visible:?}"
                );
            })
            .unwrap();

        lsp_request_ranges.lock().clear();

        // Step 1: Make an edit → triggers BufferEdited with 700ms debounce.
        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                    s.select_ranges([MultiBufferOffset(13)..MultiBufferOffset(13)])
                });
                editor.handle_input("x", window, cx);
            })
            .unwrap();
        // Let the BufferEdited event propagate and the edit task get spawned.
        cx.executor().run_until_parked();

        // Step 2: Scroll down to reveal a new chunk, then trigger NewLinesShown.
        // This spawns a scroll task with the shorter 50ms debounce.
        editor
            .update(cx, |editor, window, cx| {
                editor.scroll_screen(&ScrollAmount::Page(1.0), window, cx);
            })
            .unwrap();
        // Explicitly trigger NewLinesShown for the new visible range.
        editor
            .update(cx, |editor, _window, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();

        // Step 3: Advance clock past scroll debounce (50ms) but NOT past edit
        // debounce (700ms). The scroll task completes and adds hints for the
        // new chunk.
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        // The scroll task's apply_fetched_hints also processes
        // invalidate_hints_for_buffers (set by the earlier BufferEdited), which
        // removes the old chunk_0 hint. Only the scroll chunk's hint remains.
        editor
            .update(cx, |editor, _window, cx| {
                let visible = visible_hint_labels(editor, cx);
                assert!(
                    visible.iter().any(|h| h.starts_with("chunk_50")),
                    "After scroll task completes, the scroll chunk's hints should be \
                     present, got: {visible:?}"
                );
            })
            .unwrap();

        // Step 4: Advance clock past the edit debounce (700ms). The edit task
        // completes, calling apply_fetched_hints with should_invalidate()=true,
        // which removes ALL visible hints (including the scroll chunk's) but only
        // adds back hints for its own chunks (chunk_0).
        cx.executor().advance_clock(Duration::from_millis(700));
        cx.executor().run_until_parked();

        // At this point the edit task has:
        //   - removed chunk_50's hint (via should_invalidate removing all visible)
        //   - added chunk_0's hint (from its own fetch)
        //   - (with fix) cleared chunk_50 from hint_chunk_fetching
        // Without the fix, chunk_50 is stuck in hint_chunk_fetching and will
        // never be re-queried by NewLinesShown.

        // Step 5: Trigger NewLinesShown to give the system a chance to re-fetch
        // any chunks whose hints were lost.
        editor
            .update(cx, |editor, _window, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                let visible = visible_hint_labels(editor, cx);
                assert!(
                    visible.iter().any(|h| h.starts_with("chunk_0")),
                    "应存在 chunk_0 提示(来自编辑任务)。实际为:{visible:?}"
                );
                assert!(
                    visible.iter().any(|h| h.starts_with("chunk_50")),
                    "chunk_50 hints should have been re-fetched after NewLinesShown. \
                     Bug 1: the scroll chunk's hints were removed by the edit task \
                     and the chunk was stuck in hint_chunk_fetching, preventing \
                     re-fetch. Got: {visible:?}"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_refresh_requested_multi_server(cx: &mut gpui::TestAppContext) {
        // Bug 2: When one LSP server sends workspace/inlayHint/refresh, the editor
        // wipes all tracking state via clear(), then spawns tasks that call
        // LspStore::inlay_hints with for_server=Some(requesting_server). The LspStore
        // filters out other servers' cached hints via the for_server guard, so only
        // the requesting server's hints are returned. apply_fetched_hints removes ALL
        // visible hints (should_invalidate()=true) but only adds back the requesting
        // server's hints. Other servers' hints disappear permanently.
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                ..InlayHintSettingsContent::default()
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": "fn main() { let x = 1; } // padding to keep hints from being trimmed",
                "other.rs": "// Test file",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;
        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());

        // Server A returns a hint labeled "server_a".
        let server_a_request_count = Arc::new(AtomicU32::new(0));
        let mut fake_servers_a = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                name: "rust-analyzer",
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new({
                    let server_a_request_count = server_a_request_count.clone();
                    move |fake_server| {
                        let server_a_request_count = server_a_request_count.clone();
                        fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                            move |_params, _| {
                                let count =
                                    server_a_request_count.fetch_add(1, Ordering::Release) + 1;
                                async move {
                                    Ok(Some(vec![lsp::InlayHint {
                                        position: lsp::Position::new(0, 9),
                                        label: lsp::InlayHintLabel::String(format!(
                                            "server_a_{count}"
                                        )),
                                        kind: Some(lsp::InlayHintKind::TYPE),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    }]))
                                }
                            },
                        );
                    }
                })),
                ..FakeLspAdapter::default()
            },
        );

        // Server B returns a hint labeled "server_b" at a different position.
        let server_b_request_count = Arc::new(AtomicU32::new(0));
        let mut fake_servers_b = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                name: "secondary-ls",
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new({
                    let server_b_request_count = server_b_request_count.clone();
                    move |fake_server| {
                        let server_b_request_count = server_b_request_count.clone();
                        fake_server.set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                            move |_params, _| {
                                let count =
                                    server_b_request_count.fetch_add(1, Ordering::Release) + 1;
                                async move {
                                    Ok(Some(vec![lsp::InlayHint {
                                        position: lsp::Position::new(0, 22),
                                        label: lsp::InlayHintLabel::String(format!(
                                            "server_b_{count}"
                                        )),
                                        kind: Some(lsp::InlayHintKind::TYPE),
                                        text_edits: None,
                                        tooltip: None,
                                        padding_left: None,
                                        padding_right: None,
                                        data: None,
                                    }]))
                                }
                            },
                        );
                    }
                })),
                ..FakeLspAdapter::default()
            },
        );

        let (buffer, _buffer_handle) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let editor =
            cx.add_window(|window, cx| Editor::for_buffer(buffer, Some(project), window, cx));
        cx.executor().run_until_parked();

        let fake_server_a = fake_servers_a.next().await.unwrap();
        let _fake_server_b = fake_servers_b.next().await.unwrap();

        editor
            .update(cx, |editor, window, cx| {
                editor.set_visible_line_count(50.0, window, cx);
                editor.set_visible_column_count(120.0);
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        // Verify both servers' hints are present initially.
        editor
            .update(cx, |editor, _window, cx| {
                let visible = visible_hint_labels(editor, cx);
                let has_a = visible.iter().any(|h| h.starts_with("server_a"));
                let has_b = visible.iter().any(|h| h.starts_with("server_b"));
                assert!(
                    has_a && has_b,
                    "Both servers should have hints initially. Got: {visible:?}"
                );
            })
            .unwrap();

        // Trigger RefreshRequested from server A. This should re-fetch server A's
        // hints while keeping server B's hints intact.
        fake_server_a
            .request::<lsp::request::InlayHintRefreshRequest>((), lsp::DEFAULT_LSP_REQUEST_TIMEOUT)
            .await
            .into_response()
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        // Also trigger NewLinesShown to give the system a chance to recover
        // any chunks that might have been cleared.
        editor
            .update(cx, |editor, _window, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        editor
            .update(cx, |editor, _window, cx| {
                let visible = visible_hint_labels(editor, cx);
                let has_a = visible.iter().any(|h| h.starts_with("server_a"));
                let has_b = visible.iter().any(|h| h.starts_with("server_b"));
                assert!(
                    has_a,
                    "Server A hints should be present after its own refresh. Got: {visible:?}"
                );
                assert!(
                    has_b,
                    "Server B hints should NOT be lost when server A triggers \
                     RefreshRequested. Bug 2: clear() wipes all tracking, then \
                     LspStore filters out server B's cached hints via the for_server \
                     guard, and apply_fetched_hints removes all visible hints but only \
                     adds back server A's. Got: {visible:?}"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    async fn test_multi_language_multibuffer_no_duplicate_hints(cx: &mut gpui::TestAppContext) {
        init_test(cx, &|settings| {
            settings.defaults.inlay_hints = Some(InlayHintSettingsContent {
                show_value_hints: Some(true),
                enabled: Some(true),
                edit_debounce_ms: Some(0),
                scroll_debounce_ms: Some(0),
                show_type_hints: Some(true),
                show_parameter_hints: Some(true),
                show_other_hints: Some(true),
                show_background: Some(false),
                toggle_on_modifiers_press: None,
            })
        });

        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": "fn main() { let x = 1; } // padding to keep hints from being trimmed",
                "index.ts": "const y = 2; // padding to keep hints from being trimmed in typescript",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;
        let language_registry = project.read_with(cx, |project, _| project.languages().clone());

        let mut rs_fake_servers = None;
        let mut ts_fake_servers = None;
        for (name, path_suffix) in [("Rust", "rs"), ("TypeScript", "ts")] {
            language_registry.add(Arc::new(Language::new(
                LanguageConfig {
                    name: name.into(),
                    matcher: (LanguageMatcher {
                        path_suffixes: vec![path_suffix.to_string()],
                        ..Default::default()
                    })
                    .into(),
                    ..Default::default()
                },
                Some(tree_sitter_rust::LANGUAGE.into()),
            )));
            let fake_servers = language_registry.register_fake_lsp(
                name,
                FakeLspAdapter {
                    name,
                    capabilities: lsp::ServerCapabilities {
                        inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                        ..Default::default()
                    },
                    initializer: Some(Box::new({
                        move |fake_server| {
                            let request_count = Arc::new(AtomicU32::new(0));
                            fake_server
                                .set_request_handler::<lsp::request::InlayHintRequest, _, _>(
                                    move |params, _| {
                                        let count =
                                            request_count.fetch_add(1, Ordering::Release) + 1;
                                        let prefix = match name {
                                            "Rust" => "rs_hint",
                                            "TypeScript" => "ts_hint",
                                            other => panic!("意外的语言:{other}"),
                                        };
                                        async move {
                                            Ok(Some(vec![lsp::InlayHint {
                                                position: params.range.start,
                                                label: lsp::InlayHintLabel::String(format!(
                                                    "{prefix}_{count}"
                                                )),
                                                kind: None,
                                                text_edits: None,
                                                tooltip: None,
                                                padding_left: None,
                                                padding_right: None,
                                                data: None,
                                            }]))
                                        }
                                    },
                                );
                        }
                    })),
                    ..Default::default()
                },
            );
            match name {
                "Rust" => rs_fake_servers = Some(fake_servers),
                "TypeScript" => ts_fake_servers = Some(fake_servers),
                _ => unreachable!(),
            }
        }

        let (rs_buffer, _rs_handle) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let (ts_buffer, _ts_handle) = project
            .update(cx, |project, cx| {
                project.open_local_buffer_with_lsp(path!("/a/index.ts"), cx)
            })
            .await
            .unwrap();

        let multi_buffer = cx.new(|cx| {
            let mut multibuffer = MultiBuffer::new(Capability::ReadWrite);
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(0),
                rs_buffer.clone(),
                [Point::new(0, 0)..Point::new(1, 0)],
                0,
                cx,
            );
            multibuffer.set_excerpts_for_path(
                PathKey::sorted(1),
                ts_buffer.clone(),
                [Point::new(0, 0)..Point::new(1, 0)],
                0,
                cx,
            );
            multibuffer
        });

        cx.executor().run_until_parked();
        let editor = cx.add_window(|window, cx| {
            Editor::for_multibuffer(multi_buffer, Some(project.clone()), window, cx)
        });

        let _rs_fake_server = rs_fake_servers.unwrap().next().await.unwrap();
        let _ts_fake_server = ts_fake_servers.unwrap().next().await.unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.executor().run_until_parked();

        // Verify initial state: both languages have exactly one hint each
        editor
            .update(cx, |editor, _window, cx| {
                let visible = visible_hint_labels(editor, cx);
                let rs_hints: Vec<_> = visible
                    .iter()
                    .filter(|h| h.starts_with("rs_hint"))
                    .collect();
                let ts_hints: Vec<_> = visible
                    .iter()
                    .filter(|h| h.starts_with("ts_hint"))
                    .collect();
                assert_eq!(
                    rs_hints.len(),
                    1,
                    "最初应恰好有 1 个 Rust 提示,实际为:{rs_hints:?}"
                );
                assert_eq!(
                    ts_hints.len(),
                    1,
                    "最初应恰好有 1 个 TypeScript 提示,实际为:{ts_hints:?}"
                );
            })
            .unwrap();

        // Edit the Rust buffer — triggers BufferEdited(rust_buffer_id).
        // The language filter in refresh_inlay_hints excludes TypeScript excerpts
        // from processing, but the global clear() wipes added_hints for ALL buffers.
        editor
            .update(cx, |editor, window, cx| {
                editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                    s.select_ranges([MultiBufferOffset(0)..MultiBufferOffset(0)])
                });
                editor.handle_input("x", window, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();

        // Trigger NewLinesShown — this causes TypeScript chunks to be re-fetched
        // because hint_chunk_fetching was wiped by clear(). The cached hints pass
        // the added_hints.insert(...).is_none() filter (also wiped) and get inserted
        // alongside the still-displayed copies, causing duplicates.
        editor
            .update(cx, |editor, _window, cx| {
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();

        // Assert: TypeScript hints must NOT be duplicated
        editor
            .update(cx, |editor, _window, cx| {
                let visible = visible_hint_labels(editor, cx);
                let ts_hints: Vec<_> = visible
                    .iter()
                    .filter(|h| h.starts_with("ts_hint"))
                    .collect();
                assert_eq!(
                    ts_hints.len(),
                    1,
                    "TypeScript hints should NOT be duplicated after editing Rust buffer \
                     and triggering NewLinesShown. Got: {ts_hints:?}"
                );

                let rs_hints: Vec<_> = visible
                    .iter()
                    .filter(|h| h.starts_with("rs_hint"))
                    .collect();
                assert_eq!(
                    rs_hints.len(),
                    1,
                    "编辑后 Rust 提示仍应存在。实际为:{rs_hints:?}"
                );
            })
            .unwrap();
    }

    pub(crate) fn init_test(cx: &mut TestAppContext, f: &dyn Fn(&mut AllLanguageSettingsContent)) {
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
            theme_settings::init(theme::LoadThemes::JustBase, cx);
            release_channel::init(semver::Version::new(0, 0, 0), cx);
            crate::init(cx);
        });

        update_test_language_settings(cx, f);
    }

    async fn prepare_test_objects(
        cx: &mut TestAppContext,
        initialize: impl 'static + Send + Fn(&mut FakeLanguageServer, &'static str) + Send + Sync,
    ) -> (&'static str, WindowHandle<Editor>, FakeLanguageServer) {
        let fs = FakeFs::new(cx.background_executor.clone());
        fs.insert_tree(
            path!("/a"),
            json!({
                "main.rs": "fn main() { a } // and some long comment to ensure inlays are not trimmed out",
                "other.rs": "// Test file",
            }),
        )
        .await;

        let project = Project::test(fs, [path!("/a").as_ref()], cx).await;
        let file_path = path!("/a/main.rs");

        let language_registry = project.read_with(cx, |project, _| project.languages().clone());
        language_registry.add(rust_lang());
        let mut fake_servers = language_registry.register_fake_lsp(
            "Rust",
            FakeLspAdapter {
                capabilities: lsp::ServerCapabilities {
                    inlay_hint_provider: Some(lsp::OneOf::Left(true)),
                    ..lsp::ServerCapabilities::default()
                },
                initializer: Some(Box::new(move |server| initialize(server, file_path))),
                ..FakeLspAdapter::default()
            },
        );

        let buffer = project
            .update(cx, |project, cx| {
                project.open_local_buffer(path!("/a/main.rs"), cx)
            })
            .await
            .unwrap();
        let editor =
            cx.add_window(|window, cx| Editor::for_buffer(buffer, Some(project), window, cx));

        editor
            .update(cx, |editor, _, cx| {
                assert!(cached_hint_labels(editor, cx).is_empty());
                assert!(visible_hint_labels(editor, cx).is_empty());
            })
            .unwrap();

        cx.executor().run_until_parked();
        let fake_server = fake_servers.next().await.unwrap();

        // Establish a viewport so the editor considers itself visible and the hint refresh
        // pipeline runs. Then explicitly trigger a refresh.
        editor
            .update(cx, |editor, window, cx| {
                editor.set_visible_line_count(50.0, window, cx);
                editor.set_visible_column_count(120.0);
                editor.refresh_inlay_hints(InlayHintRefreshReason::NewLinesShown, cx);
            })
            .unwrap();
        cx.executor().run_until_parked();
        (file_path, editor, fake_server)
    }

    // Inlay hints in the cache are stored per excerpt as a key, and those keys are guaranteed to be ordered same as in the multi buffer.
    // Ensure a stable order for testing.
    fn sorted_cached_hint_labels(editor: &Editor, cx: &mut App) -> Vec<String> {
        let mut labels = cached_hint_labels(editor, cx);
        labels.sort_by(|a, b| natural_sort(a, b));
        labels
    }

    pub fn cached_hint_labels(editor: &Editor, cx: &mut App) -> Vec<String> {
        let lsp_store = editor.project().unwrap().read(cx).lsp_store();

        let mut all_cached_labels = Vec::new();
        let mut all_fetched_hints = Vec::new();
        for buffer in editor.buffer.read(cx).all_buffers() {
            lsp_store.update(cx, |lsp_store, cx| {
                let hints = lsp_store.latest_lsp_data(&buffer, cx).inlay_hints();
                all_cached_labels.extend(hints.all_cached_hints().into_iter().map(|hint| {
                    let mut label = hint.text().to_string();
                    if hint.padding_left {
                        label.insert(0, ' ');
                    }
                    if hint.padding_right {
                        label.push_str(" ");
                    }
                    label
                }));
                all_fetched_hints.extend(hints.all_fetched_hints());
            });
        }

        all_cached_labels
    }

    pub fn visible_hint_labels(editor: &Editor, cx: &Context<Editor>) -> Vec<String> {
        Editor::visible_inlay_hints(editor.display_map.read(cx))
            .map(|hint| hint.text().to_string())
            .collect()
    }

    fn allowed_hint_kinds_for_editor(editor: &Editor) -> HashSet<Option<InlayHintKind>> {
        editor
            .inlay_hints
            .as_ref()
            .unwrap()
            .allowed_hint_kinds
            .clone()
    }

    async fn run_work_cycle(
        fake_server: &FakeLanguageServer,
        progress_token: i32,
        cx: &mut gpui::TestAppContext,
    ) {
        fake_server
            .request::<lsp::request::WorkDoneProgressCreate>(
                lsp::WorkDoneProgressCreateParams {
                    token: lsp::ProgressToken::Number(progress_token),
                },
                DEFAULT_LSP_REQUEST_TIMEOUT,
            )
            .await
            .into_response()
            .expect("工作完成进度创建请求失败");
        cx.executor().run_until_parked();
        fake_server.notify::<lsp::notification::Progress>(lsp::ProgressParams {
            token: lsp::ProgressToken::Number(progress_token),
            value: lsp::ProgressParamsValue::WorkDone(lsp::WorkDoneProgress::Begin(
                lsp::WorkDoneProgressBegin::default(),
            )),
        });
        cx.executor().run_until_parked();
        fake_server.notify::<lsp::notification::Progress>(lsp::ProgressParams {
            token: lsp::ProgressToken::Number(progress_token),
            value: lsp::ProgressParamsValue::WorkDone(lsp::WorkDoneProgress::End(
                lsp::WorkDoneProgressEnd::default(),
            )),
        });
        cx.executor().run_until_parked();
    }
}
