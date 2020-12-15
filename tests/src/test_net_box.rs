use std::io;
use std::rc::Rc;
use std::time::Duration;

use tarantool_module::error::Error;
use tarantool_module::fiber::Fiber;
use tarantool_module::index::IteratorType;
use tarantool_module::net_box::{Conn, ConnOptions, Options};

use crate::common::S1Record;

pub fn test_immediate_close() {
    let _ = Conn::new("localhost:3301", ConnOptions::default()).unwrap();
}

pub fn test_ping() {
    let conn = Conn::new("localhost:3301", ConnOptions::default()).unwrap();
    conn.ping(&Options::default()).unwrap();
}

pub fn test_ping_timeout() {
    let conn = Conn::new("localhost:3301", ConnOptions::default()).unwrap();

    conn.ping(&Options {
        timeout: Some(Duration::from_millis(1)),
        ..Options::default()
    })
    .unwrap();

    conn.ping(&Options {
        timeout: None,
        ..Options::default()
    })
    .unwrap();
}

pub fn test_ping_concurrent() {
    let conn = Rc::new(Conn::new("localhost:3301", ConnOptions::default()).unwrap());

    let mut fiber_a = Fiber::new("test_fiber_a", &mut |conn: Box<Rc<Conn>>| {
        conn.ping(&Options::default()).unwrap();
        0
    });
    fiber_a.set_joinable(true);

    let mut fiber_b = Fiber::new("test_fiber_b", &mut |conn: Box<Rc<Conn>>| {
        conn.ping(&Options::default()).unwrap();
        0
    });
    fiber_b.set_joinable(true);

    fiber_a.start(conn.clone());
    fiber_b.start(conn.clone());

    fiber_a.join();
    fiber_b.join();
}

pub fn test_call() {
    let conn_options = ConnOptions {
        user: "test_user".to_string(),
        password: "password".to_string(),
        ..ConnOptions::default()
    };
    let conn = Conn::new("localhost:3301", conn_options).unwrap();
    let result = conn
        .call("test_stored_proc", &(1, 2), &Options::default())
        .unwrap();
    assert_eq!(result.unwrap().into_struct::<(i32,)>().unwrap(), (3,));
}

pub fn test_call_timeout() {
    let conn_options = ConnOptions {
        user: "test_user".to_string(),
        password: "password".to_string(),
        ..ConnOptions::default()
    };
    let conn = Conn::new("localhost:3301", conn_options).unwrap();
    let result = conn.call(
        "test_timeout",
        &Vec::<()>::new(),
        &Options {
            timeout: Some(Duration::from_millis(1)),
            ..Options::default()
        },
    );
    assert!(matches!(result, Err(Error::IO(ref e)) if e.kind() == io::ErrorKind::TimedOut));
}

pub fn test_connection_error() {
    let conn = Conn::new(
        "localhost:255",
        ConnOptions {
            reconnect_after: Duration::from_secs(0),
            ..ConnOptions::default()
        },
    )
    .unwrap();
    assert!(matches!(conn.ping(&Options::default()), Err(_)));
}

pub fn test_is_connected() {
    let conn = Conn::new(
        "localhost:3301",
        ConnOptions {
            reconnect_after: Duration::from_secs(0),
            ..ConnOptions::default()
        },
    )
    .unwrap();
    assert_eq!(conn.is_connected(), false);
    conn.ping(&Options::default()).unwrap();
    assert_eq!(conn.is_connected(), true);
}

pub fn test_schema_sync() {
    let conn = Conn::new(
        "localhost:3301",
        ConnOptions {
            user: "test_user".to_string(),
            password: "password".to_string(),
            ..ConnOptions::default()
        },
    )
    .unwrap();

    assert!(conn.space("test_s2").unwrap().is_some());
    assert!(conn.space("test_s_tmp").unwrap().is_none());

    conn.call("test_schema_update", &Vec::<()>::new(), &Options::default())
        .unwrap();
    assert!(conn.space("test_s_tmp").unwrap().is_some());

    conn.call(
        "test_schema_cleanup",
        &Vec::<()>::new(),
        &Options::default(),
    )
    .unwrap();
}

pub fn test_select() {
    let conn = Conn::new(
        "localhost:3301",
        ConnOptions {
            user: "test_user".to_string(),
            password: "password".to_string(),
            ..ConnOptions::default()
        },
    )
    .unwrap();

    let space = conn.space("test_s2").unwrap().unwrap();
    let result: Vec<S1Record> = space
        .select(IteratorType::LE, &(2,))
        .unwrap()
        .map(|x| x.into_struct().unwrap())
        .collect();

    assert_eq!(
        result,
        vec![
            S1Record {
                id: 2,
                text: "key_2".to_string()
            },
            S1Record {
                id: 1,
                text: "key_1".to_string()
            }
        ]
    );
}
