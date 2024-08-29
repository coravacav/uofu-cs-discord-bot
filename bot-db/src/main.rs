use bot_db::perform_migration;

fn main() {
    perform_migration().unwrap();
}
