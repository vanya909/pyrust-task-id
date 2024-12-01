use clap::Parser;
use regex::Regex;
use std::collections::HashMap;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::process::{exit, Command};
use strfmt::strfmt;

#[derive(Parser)]
struct Cli {
    task_regex: String,
    template: String,
    filename: String,
}

/// Run command and return output
///
/// * `full_command` - Full command with all arguments
///
/// <div class="warning">
/// This function returns an error in case when command couldn't be executed.
/// It is supposed to be used to get output from git commands.
/// </div>
fn get_git_command_output(full_command: &str) -> String {
    let (command, args) = match full_command.split_once(" ") {
        Some((first, second)) => (first, second),
        None => (full_command, ""),
    };

    let mut command = Command::new(command);

    let args: Vec<&str> = args.split(" ").collect();
    if !args.is_empty() {
        command.args(args);
    }

    let output;
    if let Ok(val) = command.output() {
        output = val;
    } else {
        eprintln!("Make sure git is installed and git repo exists. Also make sure that stage for this hook is `commit-msg`.");
        exit(1);
    }

    let output_text;
    if let Ok(val) = String::from_utf8(output.stdout) {
        output_text = val;
    } else {
        eprintln!("Got non utf-8 chars from git.");
        exit(1);
    }

    String::from(output_text.trim())
}

/// Return current git branch name if git installed and the repo exists
fn get_current_branch() -> String {
    get_git_command_output("git branch --show-current")
}

/// Return commit message's subject and body retrieved from commit message file
fn get_subject_and_body(filename: &str) -> (String, String) {
    let commit_message = read_to_string(filename).unwrap_or_default();

    // Remove comment section if presented (The line that starts from `#` is the
    // comment, the first such line is considered to be the start of comment section)
    let commit_message_last_index =
        commit_message.find("\n#").unwrap_or(commit_message.len());
    let commit_message = &commit_message[0..commit_message_last_index];

    if let Some((subject, body)) = commit_message.split_once("\n\n") {
        (subject.to_string(), body.to_string())
    } else {
        (commit_message.to_string(), String::from(""))
    }
}

/// Return task id from current branch by the given regex
///
/// * `branch_name` - Name of the branch to retrieve task id from.
/// * `regex` - `Regex` with task-id.
fn get_task_id(branch_name: &str, regex: &Regex) -> Result<String, String> {
    let regex_match = regex
        .captures(branch_name)
        .ok_or("Task id wasn't found in the branch name.")?;

    let captured_group = regex_match
        .name("task_template")
        .expect("It's not None, so can expect value.")
        .as_str();

    Ok(String::from(captured_group))
}

/// Update last commit
///
/// This function updates last commit by providing new message to it.
///
/// * `message` - message which should be provided to commit
fn update_commit_with_message(filename: &str, message: &str) {
    let mut commit_message_file =
        File::create(filename).expect("Unable to open the file.");

    commit_message_file
        .write_all(message.as_bytes())
        .expect("Unable to write data");
}

/// Format commit message
///
/// * `message_template` - Template of the result message with placeholders
/// * `commit_subject` - Subject of the last made commit
/// * `commit_body` - Body of the last made commit
/// * `task_id` - Task id that should be provided into commit message
fn format_commit_message(
    message_template: &str,
    commit_subject: &str,
    commit_body: &str,
    task_id: &str,
) -> String {
    let mut placeholders = HashMap::new();

    placeholders.insert("subject".to_string(), commit_subject);
    placeholders.insert("body".to_string(), commit_body);
    placeholders.insert("task_id".to_string(), task_id);

    if let Ok(updated_message) = strfmt(message_template, &placeholders) {
        // Replace is needed in case when body is empty and there are some
        // redundant empty lines
        updated_message.replace("\n\n\n\n", "\n\n")
    } else {
        eprintln!("Message template is incorrect. It must contain `subject`, `body` and `task_id` placeholders.");
        exit(1);
    }
}

/// Run `pyrust_task_id`
pub fn run() {
    let args = Cli::parse();

    let template = args.template.replace("\\n", "\n");

    let task_regex;
    if let Ok(val) = Regex::new(&args.task_regex) {
        task_regex = val;
    } else {
        eprintln!("Make sure task regex is correct.");
        exit(1);
    }

    let (commit_subject, commit_body) = get_subject_and_body(&args.filename);
    let branch_name = get_current_branch();

    let task_id;
    if let Ok(val) = get_task_id(&branch_name, &task_regex) {
        task_id = val;
    } else {
        // We don't want to raise error because if can't get task id from
        // branch name, it means it may be `develop` or `main` branch
        return;
    }

    if commit_subject.contains(&task_id) || commit_body.contains(&task_id) {
        return;
    }

    let updated_commit_message = format_commit_message(
        &template,
        &commit_subject,
        &commit_body,
        &task_id,
    );

    update_commit_with_message(&args.filename, &updated_commit_message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_task_id() {
        let branch_name = "feature/ABC-123-provide-tests";
        let expected = "ABC-123";

        let regex =
            Regex::new(r"feature/(?P<task_template>ABC-\d+).*").unwrap();
        let task_id = get_task_id(branch_name, &regex).unwrap();

        assert_eq!(task_id, expected);
    }
}
