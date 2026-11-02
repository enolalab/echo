use auto_subtitle_desktop::media_server;

#[tokio::test]
async fn media_server_serves_file_full_and_range() {
    media_server::start();
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("#1 v i d e o [x].mp4");
    std::fs::write(&file, b"0123456789abcdef").unwrap();

    let url = media_server::media_url(file.to_str().unwrap()).unwrap();
    assert!(url.starts_with("http://127.0.0.1:"));

    let client = reqwest::Client::new();

    // Full GET — special chars (#, spaces, brackets) must round-trip.
    let resp = client.get(&url).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers()["accept-ranges"], "bytes");
    assert_eq!(resp.headers()["content-type"], "video/mp4");
    assert_eq!(resp.bytes().await.unwrap().as_ref(), b"0123456789abcdef");

    // Range request — seeking relies on 206 partial content.
    let resp = client
        .get(&url)
        .header("Range", "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 206);
    assert_eq!(
        resp.headers()["content-range"],
        "bytes 2-5/16"
    );
    assert_eq!(resp.bytes().await.unwrap().as_ref(), b"2345");

    // Unknown token is rejected.
    let port = url.split(':').nth(2).unwrap().split('/').next().unwrap();
    let bad = format!("http://127.0.0.1:{port}/media/badtoken/{}", url.rsplit('/').next().unwrap());
    let resp = client.get(&bad).send().await.unwrap();
    assert_eq!(resp.status(), 403);

    // Missing file → 404.
    let missing = media_server::media_url("/definitely/not/here.mp4").unwrap();
    let resp = client.get(&missing).send().await.unwrap();
    assert_eq!(resp.status(), 404);
}
