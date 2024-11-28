use std::{io::Result, process::Command};

#[derive(Default)]
struct TmuxController {
    // window: Option<String>,
}

impl TmuxController {
    fn run(&self, args: &[&str]) -> Result<Vec<String>> {
        let output = Command::new("/bin/tmux").args(args).output()?;
        Ok(output
            .stdout
            .split(|c| *c == b'\n')
            .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
            .collect())
    }
    fn new(shell: Option<&str>) -> Result<Self> {
        let unset_cmd = "unset SKIM_DEFAULT_COMMAND SKIM_DEFAULT_OPTIONS PS1 PROMPT_COMMAND";

        let shell_cmd = match shell {
            None | Some("bash") => "bash --rcfile None",
            Some("zsh") => "HISTSIZE=100 zsh -f",
            Some(s) => panic!("Unknown shell {}", s)
        };

        let res = Self::default();

        let window = res.run(&[
            "new-window",
            "-d",
            "-P",
            "-F",
            "#I",
            &format!("{}; {}", unset_cmd, shell_cmd),
        ])?.first().unwrap().to_string();

        res.run(&["set-window-option", "-t", &window, "pane-base-index", "0"])?;

        Ok(res)
    }
}

#[test]
fn setup() {
    let controller = TmuxController::new(None).unwrap();
    println!("{:?}", controller.run(&["list-windows"]).unwrap());
    assert!(false);
}
