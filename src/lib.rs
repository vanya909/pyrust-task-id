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
    commit_message_template: String,
    commit_message_file: String,
}

#[derive(PartialEq, Debug)]
enum TaskIDError {
    NotInBranch,
    WrongCapturingGroup,
}

/// Return current git branch name if git installed and the repo exists
fn get_current_branch() -> String {
    let mut command = Command::new("git");
    command.args(["branch", "--show-current"]);

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

/// Return commit message's subject and body retrieved from provided message
///
/// * `commit_message` - the message that will be used to get subject and body
fn get_subject_and_body(commit_message: &str) -> (String, String) {
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
/// * `branch_name` - name of the branch to retrieve task id from
/// * `regex` - `regex` with task-id
fn get_task_id(
    branch_name: &str,
    regex: &Regex,
) -> Result<String, TaskIDError> {
    let regex_match = regex
        .captures(branch_name)
        .ok_or(TaskIDError::NotInBranch)?;

    let captured_group = regex_match
        .name("task_template")
        .ok_or(TaskIDError::WrongCapturingGroup)?
        .as_str();

    Ok(String::from(captured_group))
}

/// Update last commit
///
/// This function updates last commit by providing new message to it.
///
/// * `filename` - name of the file to write new commit message to
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
/// * `message_template` - template of the result message with placeholders
/// * `commit_subject` - subject of the last made commit
/// * `commit_body` - body of the last made commit
/// * `task_id` - task id that should be provided into commit message
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
fn provide_task_id_into_commit(
    task_regex_raw: &str,
    commit_message_template: &str,
    commit_message_filename: &str,
    branch_name: &str,
) {
    // Remove escaping for commit message template
    let template = commit_message_template.replace("\\n", "\n");

    let task_regex;
    if let Ok(val) = Regex::new(task_regex_raw) {
        task_regex = val;
    } else {
        eprintln!("Make sure task regex is correct.");
        exit(1);
    }

    let commit_message =
        read_to_string(commit_message_filename).unwrap_or_default();
    let commit_message = commit_message.trim();

    let (commit_subject, commit_body) = get_subject_and_body(commit_message);

    let task_id;
    match get_task_id(branch_name, &task_regex) {
        Ok(val) => task_id = val,
        Err(err) => match err {
            TaskIDError::WrongCapturingGroup => {
                log::warn!("Make sure you included capturing group with name `task_template`.");
                return;
            }
            // We don't want to raise error because if can't get task id from
            // branch name, it means it may be `develop` or `main` branch
            TaskIDError::NotInBranch => return,
        },
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

    update_commit_with_message(
        commit_message_filename,
        &updated_commit_message,
    );
}

/// Prase args and run
pub fn parse_args_and_run() {
    let args = Cli::parse();
    let branch_name = get_current_branch();

    provide_task_id_into_commit(
        &args.task_regex,
        &args.commit_message_template,
        &args.commit_message_file,
        &branch_name,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_task_id() {
        let branch_name = "feature/ABC-123-provide-tests";
        let expected = "ABC-123";

        let regex =
            Regex::new(r"feature/(?P<task_template>ABC-\d+).*").unwrap();
        let task_id = get_task_id(branch_name, &regex).unwrap();

        assert_eq!(task_id, expected);
    }

    #[test]
    fn test_get_subject_and_body_from_commit() {
        let expected_subject = "Commit subject";
        let expected_body = "Commit body";

        let (subject, body) =
            get_subject_and_body("Commit subject\n\nCommit body");

        assert_eq!(subject, expected_subject);
        assert_eq!(body, expected_body);
    }

    #[test]
    fn test_get_subject_and_body_from_commit_for_empty_body() {
        let expected_subject = "Commit subject";
        let expected_body = "";

        let (subject, body) = get_subject_and_body("Commit subject");

        assert_eq!(subject, expected_subject);
        assert_eq!(body, expected_body);
    }

    #[test]
    fn test_get_subject_and_body_from_commit_for_multiline_body() {
        let expected_subject = "Commit subject";
        let expected_body =
            "Commit body\nAnother line\n\nEmpty line commit body";

        let (subject, body) =
            get_subject_and_body(
                "Commit subject\n\nCommit body\nAnother line\n\nEmpty line commit body"
            );

        assert_eq!(subject, expected_subject);
        assert_eq!(body, expected_body);
    }

    #[test]
    fn test_get_task_id_without_named_capturing_group() {
        let branch_name = "feature/ABC-123-provide-tests";
        let expected = Err(TaskIDError::WrongCapturingGroup);

        let regex = Regex::new(r"feature/(ABC-\d+).*").unwrap();

        assert_eq!(get_task_id(branch_name, &regex), expected);
    }

    #[test]
    fn test_get_task_id_when_task_is_not_in_branch() {
        let branch_name = "main";
        let expected = Err(TaskIDError::NotInBranch);

        let regex =
            Regex::new(r"feature/(?P<task_template>ABC-\d+).*").unwrap();

        assert_eq!(get_task_id(branch_name, &regex), expected);
    }

    #[test]
    fn test_format_commit_message_with_subject_and_body() {
        let template = "{subject}\n\n{body}\n\n{task_id}";
        let subject = "Test commit subject";
        let body = "Test commit body";
        let task_id = "TEST-111";

        let expected = String::from(
            "Test commit subject\n\nTest commit body\n\nTEST-111",
        );

        let formatted_message =
            format_commit_message(template, subject, body, task_id);

        assert_eq!(formatted_message, expected);
    }

    #[test]
    fn test_format_commit_message_with_subject_only() {
        let template = "{subject}\n\n{body}\n\n{task_id}";
        let subject = "Test commit subject";
        let body = "";
        let task_id = "TEST-111";

        let expected = String::from("Test commit subject\n\nTEST-111");

        let formatted_message =
            format_commit_message(template, subject, body, task_id);

        assert_eq!(formatted_message, expected);
    }

    #[test]
    fn test_providing_task_id_into_commit_message() {
        let branch_name = "test/ABC-111-test";
        let task_regex = r"test/(?<task_template>ABC-\d+).*";

        let commit_message = "Commit subject\n\nCommit body";
        let commit_message_template = "{subject}\\n\\n{body}\\n\\n{task_id}";
        let expected = "Commit subject\n\nCommit body\n\nABC-111";

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", commit_message).unwrap();
        let path = file.into_temp_path();
        let path = path.to_str().unwrap();

        provide_task_id_into_commit(
            task_regex,
            commit_message_template,
            path,
            branch_name,
        );
        let commit_message = read_to_string(path).unwrap_or_default();

        assert_eq!(commit_message, expected);
    }

    #[test]
    fn test_providing_task_id_into_commit_message_without_body() {
        let branch_name = "test/ABC-111-test";
        let task_regex = r"test/(?<task_template>ABC-\d+).*";

        let commit_message = "Commit subject";
        let commit_message_template = "{subject}\\n\\n{body}\\n\\n{task_id}";
        let expected = "Commit subject\n\nABC-111";

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "{}", commit_message).unwrap();
        let path = file.into_temp_path();
        let path = path.to_str().unwrap();

        provide_task_id_into_commit(
            task_regex,
            commit_message_template,
            path,
            branch_name,
        );
        let commit_message = read_to_string(path).unwrap_or_default();

        assert_eq!(commit_message, expected);
    }

    #[test]
    fn test_providing_task_id_into_commit_message_no_task_capturing_group() {
        let branch_name = "test/ABC-111-test";
        // No named capturing group => commit message shouldn't change
        let task_regex = r"test/(ABC-\d+).*";

        let commit_message = "Commit subject\n\nCommit body";
        let commit_message_template = "{subject}\\n\\n{body}\\n\\n{task_id}";
        let expected = "Commit subject\n\nCommit body\n";

        let mut commit_message_file = NamedTempFile::new().unwrap();
        writeln!(commit_message_file, "{}", commit_message).unwrap();
        let path = commit_message_file.into_temp_path();
        let path = path.to_str().unwrap();

        provide_task_id_into_commit(
            task_regex,
            commit_message_template,
            path,
            branch_name,
        );
        let commit_message = read_to_string(path).unwrap_or_default();

        assert_eq!(commit_message, expected);
    }
}
