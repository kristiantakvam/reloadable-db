extern crate actix;
extern crate actix_web;
extern crate env_logger;
extern crate memmap;
extern crate shellexpand;

use std::fs::File;
use std::io::{Cursor, Read};

use actix::actors::signal::{ProcessSignals, Signal, SignalType, Subscribe};
use actix::registry::SystemService;
use actix::{Actor, Addr, Context, Handler};
use actix_web::http::Method;
use actix_web::middleware::Logger;
use actix_web::{server, App, HttpRequest, HttpResponse, Result};
use memmap::Mmap;

struct ReloadableDatabase {
    filepath: String,
    mmap: Mmap,
}

impl ReloadableDatabase {
    fn new(filepath: &str) -> ReloadableDatabase {
        ReloadableDatabase {
            filepath: filepath.to_string(),
            mmap: ReloadableDatabase::load_database(filepath),
        }
    }

    fn load_database(filepath: &str) -> Mmap {
        let file = File::open(filepath).unwrap();
        unsafe { Mmap::map(&file) }.unwrap()
    }

    fn reload(&mut self) {
        self.mmap = ReloadableDatabase::load_database(&self.filepath);
    }
}

impl Actor for ReloadableDatabase {
    type Context = Context<Self>;
}

impl Handler<Signal> for ReloadableDatabase {
    type Result = ();

    fn handle(&mut self, msg: Signal, _: &mut Context<Self>) {
        if msg.0 == SignalType::Hup {
            println!("SIGHUP received, reloading database");
            self.reload();
        }
    }
}

fn foo(req: &HttpRequest<ReloadableDatabase>) -> Result<String> {
    let reloadable_database = req.state();
    let database = &reloadable_database.mmap;
    let mut cursor = Cursor::new(database);
    let mut response = String::new();
    let _ = cursor.read_to_string(&mut response);

    Ok(response)
}

fn p404(_req: &HttpRequest<ReloadableDatabase>) -> HttpResponse {
    HttpResponse::NotFound()
        .content_type("text/plain")
        .body("404 Not Found")
}

fn main() {
    let bind = "127.0.0.1:8080";
    std::env::set_var("RUST_LOG", "actix_web=debug");
    env_logger::init();

    server::new(move || {
        let filepath = shellexpand::tilde("~/testdb.txt");
        let reloadable_db = ReloadableDatabase::new(&filepath);

        // Can't do this since start() consumes the ReloadableDatabase instance.
        // let addr = reloadable_db.start();
        // let signal_processor = ProcessSignals::from_registry();
        // signal_processor.do_send(Subscribe(addr.recipient()));

        App::with_state(reloadable_db)
            .middleware(Logger::default())
            .resource("/foo", |r| r.method(Method::GET).f(foo))
            .default_resource(|r| r.f(p404))
    }).bind(bind)
        .expect(&format!("Cannot bind to {}", bind))
        .run();
}
