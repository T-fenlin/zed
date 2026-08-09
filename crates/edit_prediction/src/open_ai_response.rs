pub fn text_from_response(mut res: open_ai::Response) -> Option<String> {
    let choice = res.choices.pop()?;
    let output_text = match choice.message {
        open_ai::RequestMessage::Assistant {
            content: Some(open_ai::MessageContent::Plain(content)),
            ..
        } => content,
        open_ai::RequestMessage::Assistant {
            content: Some(open_ai::MessageContent::Multipart(mut content)),
            ..
        } => {
            if content.is_empty() {
                log::error!("Baseten 补全响应无输出");
                return None;
            }

            match content.remove(0) {
                open_ai::MessagePart::Text { text } => text,
                open_ai::MessagePart::Image { .. } => {
                    log::error!("预期文本,但收到图像");
                    return None;
                }
            }
        }
        _ => {
            log::error!("无效的响应消息: {:?}", choice.message);
            return None;
        }
    };
    Some(output_text)
}
