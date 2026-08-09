use settings::SemanticTokenRules;

pub(crate) fn semantic_token_rules() -> SemanticTokenRules {
    let content = grammars::get_file("cpp/semantic_token_rules.json")
        .expect("missing cpp/semantic_token_rules.json");
    let json = std::str::from_utf8(&content.data).expect("invalid utf-8 in semantic_token_rules");
    settings::parse_json_with_comments::<SemanticTokenRules>(json)
        .expect("failed to parse cpp semantic_token_rules.json")
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, BorrowAppContext, TestAppContext};
    use language::{AutoindentMode, Buffer};
    use settings::SettingsStore;
    use std::num::NonZeroU32;
    use unindent::Unindent;

    #[gpui::test]
    async fn test_cpp_autoindent_switch_case(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    int main() {
                    switch (a) {
                    case 1:
                    b++;
                    break;
                    case 2:
                    case 3:
                    c++;
                    break;
                    default:
                    d++;
                    }
                    }
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  switch (a) {
                    case 1:
                      b++;
                      break;
                    case 2:
                    case 3:
                      c++;
                      break;
                    default:
                      d++;
                  }
                }
                "#
                .unindent(),
                "case 标签下的语句应缩进,下一个标签应取消缩进"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_access_specifier(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    class Foo {
                    public:
                    void bar();
                    private:
                    int x;
                    };
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                class Foo {
                  public:
                    void bar();
                  private:
                    int x;
                };
                "#
                .unindent(),
                "访问说明符后的成员应比说明符多缩进一级"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_access_specifier_next_line(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    class Foo {
                    public:
                      void bar();
                    void baz();
                    private:
                      int x;
                    };
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                class Foo {
                  public:
                    void bar();
                    void baz();
                  private:
                    int x;
                };
                "#
                .unindent(),
                "访问说明符后的成员应比说明符多缩进一级"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_nested_class_access_specifiers(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    class Outer {
                    public:
                    class Inner {
                    public:
                    void inner_pub();
                    private:
                    int inner_priv;
                    };
                    private:
                    int outer_priv;
                    };
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                class Outer {
                  public:
                    class Inner {
                      public:
                        void inner_pub();
                      private:
                        int inner_priv;
                    };
                  private:
                    int outer_priv;
                };
                "#
                .unindent(),
                "嵌套类的访问说明符应在每个嵌套级别独立缩进"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_consecutive_access_specifiers(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    class Foo {
                    public:
                    protected:
                    private:
                    int x;
                    };
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                class Foo {
                  public:
                  protected:
                  private:
                    int x;
                };
                "#
                .unindent(),
                "连续的访问说明符之间没有成员时,应在类级别对齐"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_indented_access_specifiers(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    class Foo {
                    int default_member;
                    public:
                    void pub_method();
                    private:
                    int priv_member;
                    };
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                class Foo {
                  int default_member;
                  public:
                    void pub_method();
                  private:
                    int priv_member;
                };
                "#
                .unindent(),
                "访问说明符应在类花括号内缩进一级"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_access_specifier_with_method_bodies(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    class Foo {
                    public:
                    void bar() {
                    if (x)
                    y++;
                    }
                    private:
                    int get_x() {
                    return x;
                    }
                    int x;
                    };
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                class Foo {
                  public:
                    void bar() {
                      if (x)
                        y++;
                    }
                  private:
                    int get_x() {
                      return x;
                    }
                    int x;
                };
                "#
                .unindent(),
                "访问说明符部分内的方法体应组合花括号和说明符的缩进"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_basic(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit([(0..0, "int main() {}")], None, cx);

            let ix = buffer.len() - 1;
            buffer.edit([(ix..ix, "\n\n")], Some(AutoindentMode::EachLine), cx);
            assert_eq!(
                buffer.text(),
                "int main() {\n  \n}",
                "花括号内的内容应缩进"
            );

            buffer
        });
    }

    #[gpui::test]
    async fn test_cpp_autoindent_if_else(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let test_settings = SettingsStore::test(cx);
            cx.set_global(test_settings);
            cx.update_global::<SettingsStore, _>(|store, cx| {
                store.update_user_settings(cx, |s| {
                    s.project.all_languages.defaults.tab_size = NonZeroU32::new(2);
                });
            });
        });
        let language = crate::language("cpp", tree_sitter_cpp::LANGUAGE.into());

        cx.new(|cx| {
            let mut buffer = Buffer::local("", cx).with_language(language, cx);

            buffer.edit(
                [(
                    0..0,
                    r#"
                    int main() {
                    if (a)
                    b;
                    }
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  if (a)
                    b;
                }
                "#
                .unindent(),
                "没有花括号的 if 语句体应缩进"
            );

            let ix = buffer.len() - 4;
            buffer.edit([(ix..ix, "\n.c")], Some(AutoindentMode::EachLine), cx);
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  if (a)
                    b
                      .c;
                }
                "#
                .unindent(),
                "字段表达式 (.c) 应比语句体缩进更多"
            );

            buffer.edit([(0..buffer.len(), "")], Some(AutoindentMode::EachLine), cx);
            buffer.edit(
                [(
                    0..0,
                    r#"
                    int main() {
                    if (a) a++;
                    else b++;
                    }
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  if (a) a++;
                  else b++;
                }
                "#
                .unindent(),
                "没有花括号的单行 if/else 应对齐在同一层级"
            );

            buffer.edit([(0..buffer.len(), "")], Some(AutoindentMode::EachLine), cx);
            buffer.edit(
                [(
                    0..0,
                    r#"
                    int main() {
                    if (a)
                    b++;
                    else
                    c++;
                    }
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  if (a)
                    b++;
                  else
                    c++;
                }
                "#
                .unindent(),
                "没有花括号的多行 if/else 应缩进语句体"
            );

            buffer.edit([(0..buffer.len(), "")], Some(AutoindentMode::EachLine), cx);
            buffer.edit(
                [(
                    0..0,
                    r#"
                    int main() {
                    if (a)
                    if (b)
                    c++;
                    }
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  if (a)
                    if (b)
                      c++;
                }
                "#
                .unindent(),
                "没有花括号的嵌套 if 语句应正确缩进"
            );

            buffer.edit([(0..buffer.len(), "")], Some(AutoindentMode::EachLine), cx);
            buffer.edit(
                [(
                    0..0,
                    r#"
                    int main() {
                    if (a)
                    b++;
                    else if (c)
                    d++;
                    else
                    f++;
                    }
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  if (a)
                    b++;
                  else if (c)
                    d++;
                  else
                    f++;
                }
                "#
                .unindent(),
                "else-if 链应将所有条件对齐在同一层级,并缩进语句体"
            );

            buffer.edit([(0..buffer.len(), "")], Some(AutoindentMode::EachLine), cx);
            buffer.edit(
                [(
                    0..0,
                    r#"
                    int main() {
                    if (a) {
                    b++;
                    } else
                    c++;
                    }
                    "#
                    .unindent(),
                )],
                Some(AutoindentMode::EachLine),
                cx,
            );
            assert_eq!(
                buffer.text(),
                r#"
                int main() {
                  if (a) {
                    b++;
                  } else
                    c++;
                }
                "#
                .unindent(),
                "混合花括号应正确缩进"
            );

            buffer
        });
    }
}
