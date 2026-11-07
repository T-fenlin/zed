# 英文目录 en-US/ 为唯一事实来源（source of truth）；本文件为简体中文译文。

add_provider = 添加服务商
key_in_env = API 密钥已由环境变量提供
key_configured = API 密钥已配置
reset_key = 重置密钥
api_key = API 密钥
api_url = API 地址
api_url_hint = 兼容 API 的基础地址。
api_key_hint = 保存在系统钥匙串中,不写入 settings.json。
models = 模型
add_model = 添加模型
model_name = 模型名称
model_name_hint = 该模型在服务商 API 中的名称。
max_completion_tokens = 最大补全 token 数
max_completion_tokens_hint = OpenAI 兼容请求允许的最大补全 token 数。
max_output_tokens = 最大输出 token 数
max_output_tokens_hint = 模型单次可输出的最大 token 数。
max_tokens = 最大 token 数
max_tokens_hint = 模型的上下文窗口大小。
remove_model = 移除模型
supports_tools = 支持工具调用
supports_images = 支持图片
supports_parallel_tool_calls = 支持 parallel_tool_calls
supports_prompt_cache_key = 支持 prompt_cache_key
supports_chat_completions = 支持 /chat/completions
uses_max_tokens = 用 max_tokens 表示输出上限
supports_thinking = 支持思考
preserves_thinking = 在历史中保留思考内容
reasoning_effort = 默认推理强度
save_provider = 保存服务商
cancel = 取消
file_exists_prompt = { $path } 已存在。要替换它吗？
file_exists_detail = 同名的文件或文件夹已存在。替换会覆盖它当前的内容。
replace = 替换
vim-argument_required = 需要参数
vim-no_file_name = 没有文件名
vim-no_file_name_detail = 写入部分缓冲区需要文件名。
vim-partial_buffer_requires_bang = 请使用 ! 写入部分缓冲区
vim-partial_buffer_requires_bang_detail = 用选中的缓冲区内容覆盖当前文件需要 '!'。
vim-failed_to_save = 保存失败
auto_update-installed_via_package_manager = Zed 是通过包管理器安装的。
auto_update-check_failed = 无法检查更新
auto_update-disabled_for_non_bundled = 未打包的应用已禁用自动更新。
feedback-copied_to_clipboard = 已复制到剪贴板
cli-should_already_be_installed = CLI 应该已经安装好了
cli-linux_path_detail =
    如果你从官方发行版安装 Zed，请把 ~/.local/bin 加入 PATH。

    如果你通过其他方式安装 Zed（例如包管理器），可能需要手动创建别名或符号链接。

    视包管理器不同，该 CLI 可能名为 zeditor、zedit、zed-editor 或其他名称。
cli-add_to_path_manually = 你可以手动把 `zed` 加入 PATH。
cli-couldnt_install_title = 无法安装 Zed CLI
cli-show_me_how = 看看怎么操作
cli-installed_toast = 已把 `zed` 安装到 { $path }。你可以在终端中启动 { $channel }。
cannot_install_the_cli = 无法安装 Zed CLI
vim-failed_to_write_lines = 写入行失败
vim-failed_to_write = :w 失败
