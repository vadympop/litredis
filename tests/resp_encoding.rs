mod common;

#[test]
fn command_encodes_single_bulk_argument() {
    assert_eq!(common::command(&["PING"]), b"*1\r\n$4\r\nPING\r\n");
}

#[test]
fn command_encodes_multiple_bulk_arguments() {
    assert_eq!(
        common::command(&["SET", "foo", "bar"]),
        b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n"
    );
}

#[test]
fn command_preserves_spaces_inside_argument() {
    assert_eq!(
        common::command(&["SET", "message", "hello world"]),
        b"*3\r\n$3\r\nSET\r\n$7\r\nmessage\r\n$11\r\nhello world\r\n"
    );
}
