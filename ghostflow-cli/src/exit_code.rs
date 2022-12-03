use std::process;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success,
    Failure,
}

pub enum Never {}

impl ExitCode {
    pub fn exit(self) -> Never {
        let code = match self {
            ExitCode::Success => 0,
            ExitCode::Failure => 1,
        };

        process::exit(code)
    }
}
