use rocksdb::DB;

pub fn get_db() -> DB {
    DB::open_default("kingfisher.db").unwrap()
}
