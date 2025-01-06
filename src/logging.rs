use log::LevelFilter;
use log4rs::{
    append::{
        console::{ConsoleAppender, Target},
        file::FileAppender,
    },
    config::{Appender, Root},
    Config,
};

pub fn init() {
    log4rs::init_config(
        Config::builder()
            .appender(Appender::builder().build(
                "stderr",
                Box::new(ConsoleAppender::builder().target(Target::Stderr).build()),
            ))
            .appender(Appender::builder().build(
                "file",
                Box::new(FileAppender::builder().build("/tmp/containr.log").unwrap()),
            ))
            .build(
                Root::builder()
                    .appenders(["stderr", "file"])
                    .build(LevelFilter::Trace),
            )
            .unwrap(),
    )
    .unwrap();
}
