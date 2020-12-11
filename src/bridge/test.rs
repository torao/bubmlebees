use url::Url;

#[test]
fn test_url() {
  let url = Url::parse("tcp://username:pass@127.0.0.1:8899/root/path?key=value").unwrap();
  assert_eq!("tcp", url.scheme());
  assert_eq!("username", url.username());
  assert_eq!("pass", url.password().unwrap());
  assert_eq!("127.0.0.1", url.host().unwrap().to_string());
  assert_eq!(8899u16, url.port().unwrap());
  assert_eq!("/root/path", url.path());
  assert_eq!("key=value", url.query().unwrap());
}
