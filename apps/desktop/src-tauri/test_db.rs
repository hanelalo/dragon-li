use rusqlite::Connection;

fn main() {
    let conn = Connection::open(std::env::var("HOME").unwrap() + "/.dragon-li/data/dragon_li.db").unwrap();
    let mut stmt = conn.prepare("SELECT id, deleted_at FROM sessions;").unwrap();
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    }).unwrap();
    for row in rows {
        println!("{:?}", row.unwrap());
    }
}
