use i::browser::input::user_content;
use serde_json::json;

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=";

#[test]
fn optional_caption_and_image_only_have_actual_image_blocks() {
    let urls = vec![PNG.to_owned()];
    assert_eq!(user_content("caption", &urls).unwrap()[0], json!({"type":"text","text":"caption"}));
    let only = user_content("", &urls).unwrap();
    assert_eq!(only.as_array().unwrap().len(), 1);
    assert_eq!(only[0]["image_url"]["url"], PNG);
    assert_eq!(user_content("plain", &[]).unwrap(), json!("plain"));
}

#[test]
fn fake_images_remote_urls_and_empty_messages_are_rejected() {
    assert!(user_content("", &[]).is_err());
    for image in ["https://example.test/a.png", "data:image/png;base64,aGVsbG8=", "data:image/jpeg;base64,iVBORw0KGgo="]
    {
        assert!(user_content("caption", &[image.into()]).is_err());
    }
    assert!(user_content("caption", &vec![PNG.into(); 9]).is_err());
}
