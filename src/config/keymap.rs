use crate::domain::keymap::{HelpCategory, HelpContext, KeyBinding};

pub fn default_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::primary(
            HelpContext::Explorer,
            HelpCategory::Navigation,
            &["j", "Down"],
            "下移",
            "在列表中下移",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::Navigation,
            &["k", "Up"],
            "上移",
            "在列表中上移",
        ),
        KeyBinding::primary(
            HelpContext::Explorer,
            HelpCategory::OpenClose,
            &["Enter"],
            "打开",
            "进入目录或打开全屏阅读",
        ),
        KeyBinding::primary(
            HelpContext::Explorer,
            HelpCategory::OpenClose,
            &["Backspace", "h"],
            "返回",
            "返回上级目录",
        ),
        KeyBinding::primary(
            HelpContext::Explorer,
            HelpCategory::SearchFilter,
            &["/"],
            "搜索",
            "按名称、备注和摘要过滤",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::SearchFilter,
            &["Esc"],
            "清除过滤",
            "清除当前已应用的搜索过滤",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::LayoutFocus,
            &["Tab"],
            "切换焦点",
            "在列表、预览和信息面板之间切换",
        ),
        KeyBinding::primary(
            HelpContext::Explorer,
            HelpCategory::NotesBookmarks,
            &["m"],
            "备注",
            "编辑当前项备注",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::NotesBookmarks,
            &["p"],
            "收藏",
            "收藏当前目录或当前选中的目录",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::NotesBookmarks,
            &["B"],
            "书签列表",
            "打开书签选择器",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::PreviewAction,
            &["o"],
            "外部打开",
            "用系统默认程序打开",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::SearchFilter,
            &["."],
            "隐藏文件",
            "切换隐藏文件显示",
        ),
        KeyBinding::new(
            HelpContext::Explorer,
            HelpCategory::Advanced,
            &[":"],
            "命令行",
            "打开底部命令行",
        ),
        KeyBinding::primary(
            HelpContext::Explorer,
            HelpCategory::Advanced,
            &["?", "F1"],
            "帮助",
            "打开帮助覆盖层",
        ),
        KeyBinding::primary(
            HelpContext::Explorer,
            HelpCategory::OpenClose,
            &["q"],
            "退出",
            "退出当前界面",
        ),
        KeyBinding::primary(
            HelpContext::Preview,
            HelpCategory::Navigation,
            &["j", "k", "Down", "Up"],
            "滚动",
            "滚动预览区域",
        ),
        KeyBinding::new(
            HelpContext::Preview,
            HelpCategory::Navigation,
            &["PageDown", "PageUp"],
            "翻页",
            "按页滚动预览",
        ),
        KeyBinding::new(
            HelpContext::Preview,
            HelpCategory::Navigation,
            &["H", "L"],
            "横移",
            "左右横向滚动长行内容",
        ),
        KeyBinding::new(
            HelpContext::Preview,
            HelpCategory::OpenClose,
            &["Enter"],
            "阅读",
            "打开全屏阅读视图",
        ),
        KeyBinding::primary(
            HelpContext::Preview,
            HelpCategory::PreviewAction,
            &["n"],
            "行号",
            "切换代码行号显示",
        ),
        KeyBinding::primary(
            HelpContext::Preview,
            HelpCategory::PreviewAction,
            &["o"],
            "外部打开",
            "用系统默认程序打开当前文件",
        ),
        KeyBinding::primary(
            HelpContext::Preview,
            HelpCategory::LayoutFocus,
            &["Tab"],
            "切换焦点",
            "切到信息面板或返回列表",
        ),
        KeyBinding::new(
            HelpContext::Preview,
            HelpCategory::LayoutFocus,
            &["h", "Left"],
            "返回列表",
            "把焦点切回文件列表",
        ),
        KeyBinding::new(
            HelpContext::Preview,
            HelpCategory::SearchFilter,
            &["Esc"],
            "清除过滤",
            "清除当前已应用的搜索过滤",
        ),
        KeyBinding::primary(
            HelpContext::Preview,
            HelpCategory::Advanced,
            &["?", "F1"],
            "帮助",
            "打开帮助覆盖层",
        ),
        KeyBinding::primary(
            HelpContext::Preview,
            HelpCategory::OpenClose,
            &["q"],
            "退出",
            "退出当前界面",
        ),
        KeyBinding::new(
            HelpContext::Inspector,
            HelpCategory::Navigation,
            &["j", "k", "Down", "Up"],
            "滚动",
            "滚动当前信息面板",
        ),
        KeyBinding::new(
            HelpContext::Inspector,
            HelpCategory::PreviewAction,
            &["n"],
            "行号",
            "切换代码行号显示",
        ),
        KeyBinding::new(
            HelpContext::Inspector,
            HelpCategory::SearchFilter,
            &["Esc"],
            "清除过滤",
            "清除当前已应用的搜索过滤",
        ),
        KeyBinding::new(
            HelpContext::Inspector,
            HelpCategory::OpenClose,
            &["Enter"],
            "阅读",
            "打开全屏阅读视图",
        ),
        KeyBinding::primary(
            HelpContext::Reader,
            HelpCategory::Navigation,
            &["j", "k", "Down", "Up"],
            "滚动",
            "滚动全屏阅读视图",
        ),
        KeyBinding::new(
            HelpContext::Reader,
            HelpCategory::Navigation,
            &["PageDown", "PageUp"],
            "翻页",
            "按页滚动阅读视图",
        ),
        KeyBinding::new(
            HelpContext::Reader,
            HelpCategory::Navigation,
            &["Left", "Right", "H", "L"],
            "横移",
            "左右横向滚动长行内容",
        ),
        KeyBinding::primary(
            HelpContext::Reader,
            HelpCategory::PreviewAction,
            &["n"],
            "行号",
            "切换代码行号显示",
        ),
        KeyBinding::new(
            HelpContext::Reader,
            HelpCategory::PreviewAction,
            &["o"],
            "外部打开",
            "用系统默认程序打开当前文件",
        ),
        KeyBinding::primary(
            HelpContext::Reader,
            HelpCategory::Advanced,
            &["?", "F1"],
            "帮助",
            "打开帮助覆盖层",
        ),
        KeyBinding::primary(
            HelpContext::Reader,
            HelpCategory::OpenClose,
            &["Esc", "q"],
            "关闭",
            "关闭阅读视图",
        ),
        KeyBinding::new(
            HelpContext::OverlayHelp,
            HelpCategory::OpenClose,
            &["Esc", "q", "?"],
            "关闭帮助",
            "关闭帮助覆盖层",
        ),
        KeyBinding::new(
            HelpContext::OverlayHelp,
            HelpCategory::Navigation,
            &["Up", "Down", "PageDown", "PageUp"],
            "滚动",
            "滚动帮助内容",
        ),
        KeyBinding::new(
            HelpContext::OverlayHelp,
            HelpCategory::LayoutFocus,
            &["Left", "Right", "Tab", "Shift-Tab"],
            "切换分类",
            "切换当前帮助分类",
        ),
        KeyBinding::new(
            HelpContext::OverlayHelp,
            HelpCategory::SearchFilter,
            &["直接输入", "Backspace"],
            "过滤",
            "按关键字过滤帮助项",
        ),
        KeyBinding::primary(
            HelpContext::OverlayNoteEditor,
            HelpCategory::Editing,
            &["Enter"],
            "保存",
            "保存备注",
        ),
        KeyBinding::primary(
            HelpContext::OverlayNoteEditor,
            HelpCategory::OpenClose,
            &["Esc"],
            "取消",
            "取消编辑并关闭备注层",
        ),
        KeyBinding::primary(
            HelpContext::OverlayNoteEditor,
            HelpCategory::Editing,
            &["Ctrl-U"],
            "清空",
            "清空当前备注内容",
        ),
        KeyBinding::primary(
            HelpContext::OverlayNoteEditor,
            HelpCategory::Advanced,
            &["?", "F1"],
            "帮助",
            "打开帮助覆盖层",
        ),
        KeyBinding::primary(
            HelpContext::BookmarkPicker,
            HelpCategory::Navigation,
            &["j", "k", "Down", "Up"],
            "选择",
            "上下选择书签",
        ),
        KeyBinding::primary(
            HelpContext::BookmarkPicker,
            HelpCategory::OpenClose,
            &["Enter"],
            "跳转",
            "打开选中的书签目录",
        ),
        KeyBinding::primary(
            HelpContext::BookmarkPicker,
            HelpCategory::NotesBookmarks,
            &["d"],
            "删除",
            "删除当前书签",
        ),
        KeyBinding::primary(
            HelpContext::BookmarkPicker,
            HelpCategory::OpenClose,
            &["Esc"],
            "关闭",
            "关闭书签选择器",
        ),
        KeyBinding::primary(
            HelpContext::BookmarkPicker,
            HelpCategory::Advanced,
            &["?", "F1"],
            "帮助",
            "打开帮助覆盖层",
        ),
        KeyBinding::new(
            HelpContext::OverlayConfirm,
            HelpCategory::OpenClose,
            &["Enter", "y"],
            "确认",
            "确认当前操作",
        ),
        KeyBinding::new(
            HelpContext::OverlayConfirm,
            HelpCategory::OpenClose,
            &["Esc", "n"],
            "取消",
            "取消当前操作",
        ),
        KeyBinding::new(
            HelpContext::OverlayConfirm,
            HelpCategory::Advanced,
            &["?", "F1"],
            "帮助",
            "打开帮助覆盖层",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::Advanced,
            &[":help"],
            "帮助",
            "显示帮助页",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::NotesBookmarks,
            &[":bookmark add [name]"],
            "添加书签",
            "保存当前目录书签",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::NotesBookmarks,
            &[":bookmark jump NAME"],
            "跳转书签",
            "跳转到命名书签",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::NotesBookmarks,
            &[":bookmark list"],
            "书签列表",
            "打开书签选择器",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::NotesBookmarks,
            &[":note edit"],
            "编辑备注",
            "打开备注编辑层",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::NotesBookmarks,
            &[":note clear"],
            "清空备注",
            "删除当前项备注",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::SearchFilter,
            &[":sort name"],
            "名称排序",
            "按名称排序",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::SearchFilter,
            &[":sort mtime"],
            "时间排序",
            "按修改时间排序",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::SearchFilter,
            &[":toggle hidden"],
            "隐藏文件",
            "切换隐藏文件显示",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::PreviewAction,
            &[":inspect"],
            "阅读视图",
            "打开当前项全屏阅读",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::PreviewAction,
            &[":toggle numbers"],
            "切换行号",
            "切换代码行号显示",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::PreviewAction,
            &[":open external"],
            "外部打开",
            "使用系统默认程序打开",
        ),
        KeyBinding::new(
            HelpContext::CommandLine,
            HelpCategory::OpenClose,
            &[":quit"],
            "退出",
            "退出当前界面",
        ),
    ]
}

pub fn context_label(context: HelpContext) -> &'static str {
    match context {
        HelpContext::Explorer => "浏览",
        HelpContext::Preview => "预览",
        HelpContext::Inspector => "信息",
        HelpContext::Reader => "阅读",
        HelpContext::OverlayHelp => "帮助",
        HelpContext::OverlayNoteEditor => "备注编辑",
        HelpContext::OverlayConfirm => "确认",
        HelpContext::CommandLine => "命令",
        HelpContext::BookmarkPicker => "书签",
    }
}

pub fn category_label(category: HelpCategory) -> &'static str {
    match category {
        HelpCategory::Navigation => "基础导航",
        HelpCategory::OpenClose => "打开与返回",
        HelpCategory::SearchFilter => "搜索与筛选",
        HelpCategory::NotesBookmarks => "备注与书签",
        HelpCategory::PreviewAction => "预览与动作",
        HelpCategory::LayoutFocus => "布局与焦点",
        HelpCategory::Editing => "编辑",
        HelpCategory::Advanced => "高级动作",
    }
}

pub fn render_help_keys_lines() -> Vec<String> {
    let mut lines = vec!["lsz -l 快捷键帮助".to_string(), String::new()];
    for binding in default_key_bindings() {
        lines.push(format!(
            "[{} / {}] {:20} {:12} {}",
            context_label(binding.context),
            category_label(binding.category),
            binding.keys.join(", "),
            binding.label,
            binding.detail
        ));
    }
    lines
}

pub fn print_help_keys() {
    for line in render_help_keys_lines() {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_document_uses_same_binding_source() {
        let lines = render_help_keys_lines();
        assert_eq!(lines.len(), default_key_bindings().len() + 2);
        assert!(lines.iter().any(|line| line.contains("基础导航")));
        assert!(lines.iter().any(|line| line.contains("备注与书签")));
    }
}
