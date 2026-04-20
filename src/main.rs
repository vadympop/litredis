mod config;
mod server;

fn main() {
    env_logger::init();
    log::info!("redis-app starting");
}
