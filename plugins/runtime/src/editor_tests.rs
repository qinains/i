use super::*;
use crate::console_editor::LIMIT;
#[test]
fn long_paste_is_a_compact_token_and_submits_full_text_with_caption() {
    let mut editor = Editor::default();
    let pasted = "中文\n".repeat(500);
    assert!(editor.paste(&pasted));
    assert!(editor.text.contains("[Pasted Content"));
    assert!(editor.text.len() < 100);
    assert!(editor.insert(" explain this"));
    assert_eq!(editor.submit(), format!("{pasted} explain this"));
}
#[test]
fn short_multiline_paste_folds_without_losing_lines() {
    let mut editor = Editor::default();
    editor.paste("one\ntwo");
    assert!(editor.text.contains("Pasted Content"));
    assert_eq!(editor.submit(), "one\ntwo");
}
#[test]
fn folded_paste_budget_and_atomic_backspace_do_not_leave_hidden_content() {
    let mut editor = Editor::default();
    assert!(editor.paste(&"a".repeat(LIMIT - 10)));
    assert!(!editor.insert(&"b".repeat(20)));
    editor.edit(Key::Backspace);
    assert_eq!(editor.text, "");
    assert_eq!(editor.submit(), "");
}
#[test]
fn pending_image_waits_for_enter_and_allows_an_optional_caption() {
    let mut ui = decoder();
    let bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    ui.stage_image(bytes.clone()).unwrap();
    assert_eq!(ui.editor.text, "[Image #1]");
    assert!(ui.pending.is_empty());
    let (sender, input) = mpsc::sync_channel(1);
    ui.input = input;
    sender.send(RawInput::Bytes(b"\r".to_vec())).unwrap();
    assert!(
        matches!(ui.next(Duration::ZERO).unwrap(),Some(Input::Attachments{line,images}) if line.trim().is_empty()&&images==vec![bytes])
    );
}
#[test]
fn clipboard_image_is_requested_only_by_an_explicit_ctrl_v_key() {
    let mut keys = Keys::default();
    assert_eq!(keys.feed(&[22]), vec![Key::Clipboard]);
    let mut editor = Editor::default();
    assert!(editor.paste("[Image #1]"));
    assert_eq!(editor.submit(), "[Image #1]");
}
#[test]
fn image_caption_does_not_rewrite_token_like_text_inside_a_folded_paste() {
    let mut ui = decoder();
    ui.stage_image(vec![137, 80, 78, 71, 13, 10, 26, 10]).unwrap();
    let text = format!("{}[Image #1]", "x".repeat(1000));
    ui.editor.paste(&text);
    let (sender, input) = mpsc::sync_channel(1);
    ui.input = input;
    sender.send(RawInput::Bytes(b"\r".to_vec())).unwrap();
    assert!(matches!(ui.next(Duration::ZERO).unwrap(),Some(Input::Attachments{line,..}) if line==text));
}
#[test]
fn slash_menu_filters_navigates_and_fills_without_submission() {
    let mut editor = Editor::default();
    editor.insert("/");
    assert!(editor.suggestions().iter().any(|entry| entry.0 == "/help"));
    assert!(!editor.suggestions().iter().any(|entry| entry.0 == "/approve"));
    editor.insert("mo");
    assert_eq!(editor.suggestions()[0].0, "/mode");
    assert!(editor.completion_key(&Key::Enter));
    assert_eq!(editor.text, "/mode ");
    assert_eq!(editor.suggestions().len(), 3);
    assert!(editor.completion_key(&Key::Next));
    assert!(editor.completion_key(&Key::Tab));
    assert_eq!(editor.text, "/mode auto");
    assert!(!editor.completion_key(&Key::Enter));
    assert_eq!(editor.submit(), "/mode auto");
}
#[test]
fn slash_menu_ignores_prose_paste_and_dismisses_until_edit() {
    let mut editor = Editor::default();
    editor.insert("path /mo");
    assert!(editor.suggestions().is_empty());
    editor.submit();
    editor.insert("/");
    assert!(editor.completion_key(&Key::Escape));
    assert!(editor.suggestions().is_empty());
    assert!(editor.completion_key(&Key::Escape));
    assert!(editor.suggestions().is_empty());
    editor.insert("mo");
    assert!(!editor.suggestions().is_empty());
    editor.submit();
    editor.paste("/exit\n/help");
    assert!(editor.suggestions().is_empty());
    assert_eq!(editor.submit(), "/exit\n/help");
}

#[test]
fn fully_typed_slash_argument_submits_without_a_second_enter() {
    let mut editor = Editor::default();
    editor.insert("/mode auto");
    assert!(!editor.completion_key(&Key::Enter));
    assert_eq!(editor.submit(), "/mode auto");
}
#[test]
fn slash_menu_approval_context_and_escape_timeout() {
    let mut editor = Editor::default();
    editor.insert("/a");
    assert!(editor.suggestions().is_empty());
    editor.approval_pending = true;
    assert_eq!(editor.suggestions()[0].0, "/approve");
    let mut keys = Keys::default();
    assert!(keys.feed(b"\x1b").is_empty());
    thread::sleep(Duration::from_millis(110));
    assert!(keys.expire_escape());
    assert!(!keys.expire_escape());
}
#[test]
fn raw_menu_enter_fills_then_requires_a_second_enter_to_submit() {
    let mut ui = decoder();
    let (sender, input) = mpsc::sync_channel(4);
    ui.input = input;
    sender.send(RawInput::Bytes(b"/ex\r".to_vec())).unwrap();
    assert!(ui.next(Duration::ZERO).unwrap().is_none());
    assert_eq!(ui.editor.text, "/exit");
    sender.send(RawInput::Bytes(b"\r".to_vec())).unwrap();
    assert!(matches!(ui.next(Duration::ZERO).unwrap(), Some(Input::Line(line)) if line == "/exit"));
    sender.send(RawInput::Bytes(b"\x1b[200~/exit\n/help\x1b[201~".to_vec())).unwrap();
    assert!(ui.next(Duration::ZERO).unwrap().is_none());
    assert!(ui.editor.text.contains("Pasted Content"));
}
#[test]
fn chinese_backspace_and_delete_remove_whole_scalars() {
    let mut editor = Editor::default();
    assert!(editor.insert("你好"));
    editor.edit(Key::Backspace);
    assert_eq!(editor.text, "你");
    assert_eq!(editor.cursor, 3);
    editor.edit(Key::Backspace);
    editor.edit(Key::Backspace);
    assert_eq!(editor.text, "");
    editor.insert("中文");
    editor.edit(Key::Home);
    editor.edit(Key::Delete);
    assert_eq!(editor.text, "文");
    editor.edit(Key::Delete);
    editor.edit(Key::Delete);
    assert_eq!(editor.text, "");
}
#[test]
fn insertion_navigation_and_history_preserve_the_draft() {
    let mut editor = Editor::default();
    editor.insert("你好");
    editor.edit(Key::Left);
    editor.insert("，");
    assert_eq!(editor.text, "你，好");
    assert_eq!(editor.cursor, 6);
    editor.edit(Key::Home);
    editor.edit(Key::Right);
    editor.edit(Key::Delete);
    assert_eq!(editor.submit(), "你好");
    editor.insert("未提交");
    editor.edit(Key::Previous);
    assert_eq!(editor.text, "你好");
    editor.edit(Key::Next);
    assert_eq!(editor.text, "未提交");
    editor.edit(Key::Home);
    editor.edit(Key::Kill);
    assert!(editor.text.is_empty());
}
#[test]
fn approvals_do_not_enter_input_history() {
    let mut editor = Editor::default();
    editor.insert("任务");
    editor.submit();
    for response in ["y", "n", "Y", "/approve", "/deny", "/mode auto", "/mode ask", "/mode status"] {
        editor.insert(response);
        editor.submit();
    }
    editor.edit(Key::Previous);
    assert_eq!(editor.text, "任务");
}
#[test]
fn utf8_and_navigation_sequences_can_be_split_across_reads() {
    let mut keys = Keys::default();
    assert!(keys.feed(&[0xe4]).is_empty());
    assert!(keys.feed(&[0xbd]).is_empty());
    assert_eq!(keys.feed(&[0xa0]), vec![Key::Text("你".into())]);
    assert!(keys.feed(b"\x1b[").is_empty());
    assert_eq!(keys.feed(b"D"), vec![Key::Left]);
    assert_eq!(
        keys.feed(b"\x7f\x1b[3~\x01\x05\x03"),
        vec![Key::Backspace, Key::Delete, Key::Home, Key::End, Key::Cancel]
    );
}
#[test]
fn bracketed_paste_never_submits_embedded_newlines_or_control_c() {
    let mut keys = Keys::default();
    assert!(keys.feed(b"\x1b[200~").is_empty());
    assert!(keys.feed("中文\r\n/exit\u{3}".as_bytes()).is_empty());
    assert!(keys.feed(b"\x1b[20").is_empty());
    assert_eq!(keys.feed(b"1~"), vec![Key::Paste("中文\n/exit\u{3}".into())]);
    let mut editor = Editor::default();
    editor.insert("中文\n/exit\u{3}");
    assert_eq!(editor.text, "中文\n/exit");
    assert_eq!(keys.feed(b"\r"), vec![Key::Enter]);
}
#[test]
fn oversized_paste_is_rejected_not_split_into_commands() {
    let mut keys = Keys::default();
    keys.feed(b"\x1b[200~");
    keys.feed(&vec![b'a'; LIMIT + 20]);
    assert_eq!(keys.feed(b"\x1b[201~"), vec![Key::TooLong]);
    assert_eq!(keys.feed("好".as_bytes()), vec![Key::Text("好".into())]);
    let mut editor = Editor::default();
    assert!(editor.insert("原文"));
    assert!(!editor.insert(&"x".repeat(LIMIT)));
    assert_eq!(editor.text, "原文");
}
#[test]
fn viewport_keeps_chinese_cursor_in_bounds_without_wrapping() {
    let mut editor = Editor::default();
    editor.insert(&"你好abc".repeat(40));
    for columns in 1..100 {
        for _ in 0..5 {
            let (text, cursor) = view(&editor, "Allow? [y/N] ", columns);
            let width: usize = text.chars().map(|c| glyph(c).1).sum();
            assert!(width < columns);
            assert!(cursor < columns);
            assert!(text.is_char_boundary(text.len()));
            editor.edit(Key::Left);
        }
    }
}
#[test]
fn special_glyphs_are_escaped_for_display_but_submitted_unchanged() {
    let text = "a\u{301}👩\u{200d}💻";
    let mut editor = Editor::default();
    editor.insert(text);
    let (shown, _) = view(&editor, "", 100);
    assert!(shown.contains("\\u{301}"));
    assert!(shown.contains("\\u{200d}"));
    assert!(!shown.contains('👩'));
    assert_eq!(editor.submit(), text);
}
#[test]
fn repeated_chinese_delete_cycles_return_to_the_empty_prompt() {
    let mut editor = Editor::default();
    for _ in 0..100 {
        editor.insert("中文");
        assert_eq!(view(&editor, "", 80).1, 11);
        editor.edit(Key::Backspace);
        assert_eq!(view(&editor, "", 80).1, 9);
        editor.edit(Key::Backspace);
        assert_eq!(view(&editor, "", 80), ("lninl> ".into(), 7));
    }
}
fn decoder() -> Ui {
    let (sender, output) = mpsc::sync_channel(128);
    let (_, input) = mpsc::sync_channel(1);
    Ui {
        mode: None,
        sender,
        output,
        input,
        pending: VecDeque::new(),
        closed: false,
        keys: Keys::default(),
        editor: Editor::default(),
        utf8: [Vec::new(), Vec::new()],
        markdown: false,
        markdown_fence: None,
        markdown_line: Vec::new(),
        markdown_style: markdown::Style { color: false },
        tail: String::new(),
        drawn: String::new(),
        cursor: 0,
        columns: 80,
        menu_rows: 0,
        images: Vec::new(),
    }
}
#[test]
fn background_utf8_is_decoded_per_stream_and_terminal_controls_are_inert() {
    let mut ui = decoder();
    ui.decode(0, &[0xe4], false).unwrap();
    ui.decode(1, b"err", false).unwrap();
    ui.decode(0, &[0xbd, 0xa0], false).unwrap();
    assert_eq!(ui.tail, "err你");
    ui.decode(1, b"\x1b[2J", false).unwrap();
    assert!(ui.tail.ends_with("\\u{1b}[2J"));
    assert!(!ui.tail.contains('\x1b'));
    ui.decode(0, &[0xe4], false).unwrap();
    ui.decode(0, &[], true).unwrap();
    assert!(ui.tail.ends_with('\u{fffd}'));
}
#[test]
fn resize_reflow_accounts_for_wide_character_wrap_gaps() {
    assert_eq!(reflow_rows("abc你好", 7, 4), 2);
    assert_eq!(reflow_rows("lninl> 你好", 11, 80), 0);
}
