use xterm_backend::controllers::static_files::inject_token_into_html;

#[test]
fn test_inject_token_into_html() {
    let original_html = "<!DOCTYPE html><html><head><title>TermDeck</title></head><body></body></html>";
    let token = "test-session-token-12345";
    let injected = inject_token_into_html(original_html, token);

    assert!(injected.contains(r#"<meta name="termdeck-token" content="test-session-token-12345">"#));
    assert!(injected.contains("<title>TermDeck</title>"));
}
