use crate::{
    cli::{args::Length, io::read_hidden_input},
    store::PasswordStore,
};
use anyhow::anyhow;
use clipboard::{ClipboardContext, ClipboardProvider};
use colored::*;
use passwords::PasswordGenerator;
use std::io::Write;

use super::io::PromptPassword;

pub fn copy_to_clipboard(password: String) -> anyhow::Result<()> {
    let ctx_result: Result<ClipboardContext, _> = ClipboardProvider::new();
    let mut ctx = ctx_result.map_err(|_| anyhow!("Unable to initialize clipboard"))?;
    ctx.set_contents(password)
        .map_err(|_| anyhow!("Unable to set clipboard contents"))?;
    Ok(())
}

// TODO: Refactor this code to pass fewer arguments
#[allow(clippy::too_many_arguments)]
pub fn add_password(
    writer: &mut dyn Write,
    prompt_password: &dyn PromptPassword,
    password_store: &mut PasswordStore,
    service: String,
    username: Option<String>,
    password: Option<String>,
    generate: bool,
    password_generator: PasswordGenerator,
) -> anyhow::Result<()> {
    let password = if generate {
        let password = password_generator
            .generate_one()
            .unwrap_or_else(|_| panic!("{}", "Failed to generate password".red()));
        match copy_to_clipboard(password.clone()) {
            Ok(_) => writeln!(writer, "Random password generated and copied to clipboard")?,
            Err(err) => {
                writeln!(writer, "Random password generated")?;
                writeln!(
                    writer,
                    "Note: Failed to copy password to clipboard: {}",
                    err
                )?;
            }
        }
        password
    } else {
        password.unwrap_or_else(|| read_hidden_input("password", prompt_password))
    };
    password_store
        .load()?
        .push(service, username, password)?
        .dump()?;
    Ok(())
}

pub fn generate_password(
    length: Length,
    symbols: bool,
    uppercase: bool,
    lowercase: bool,
    numbers: bool,
    count: usize,
    writer: &mut dyn Write,
) -> anyhow::Result<()> {
    let password_generator = PasswordGenerator::new()
        .length(length.get_val())
        .lowercase_letters(lowercase)
        .uppercase_letters(uppercase)
        .numbers(numbers)
        .symbols(symbols)
        .strict(true);
    writeln!(writer)?;
    if count > 1 {
        match password_generator.generate(count) {
            Ok(passwords) => {
                for password in passwords {
                    writeln!(writer, "{}", password.green())?
                }
            }
            Err(err) => writeln!(
                writer,
                "{}",
                format!("Error generating password: {}", err).red()
            )?,
        }
    } else {
        match password_generator.generate_one() {
            Ok(password) => {
                writeln!(writer, "{}", password.green())?;
                match copy_to_clipboard(password) {
                    Ok(_) => writeln!(writer, "(Random password generated. Copied to clipboard)")?,
                    Err(err) => {
                        writeln!(
                            writer,
                            "{}",
                            format!("(Random password generated. Failed to copy password to clipboard: {})", err).yellow()
                        )?;
                    }
                }
            }
            Err(err) => writeln!(
                writer,
                "{}",
                format!("Error generating password: {}", err).red()
            )?,
        }
    }
    Ok(())
}

pub fn show_password(
    password_store: &mut PasswordStore,
    service: String,
    username: Option<String>,
    writer: &mut dyn Write,
) -> anyhow::Result<()> {
    let password = password_store.load()?.find(service, username);
    if let Some(password) = password {
        password.print_password(Some(Color::Blue), writer)?;
    } else {
        writeln!(writer, "Password not found")?;
    }
    Ok(())
}

pub fn list_passwords(
    password_store: &mut PasswordStore,
    show_passwords: bool,
    writer: &mut dyn Write,
) -> anyhow::Result<()> {
    password_store
        .load()?
        .print(show_passwords, Some(Color::Blue), writer);
    Ok(())
}

pub fn remove_password<W: Write>(
    writer: &mut W,
    password_store: &mut PasswordStore,
    service: String,
    username: Option<String>,
) -> anyhow::Result<()> {
    password_store
        .load()?
        .pop(writer, service, username)
        .dump()?;
    Ok(())
}

pub fn update_master_password<W: Write>(
    writer: &mut W,
    new_master_password: String,
    password_store: &mut PasswordStore,
) -> anyhow::Result<()> {
    password_store
        .load()?
        .update_master(new_master_password)
        .dump()?;
    writeln!(writer, "{}", "Master password updated successfully".green())
        .unwrap_or_else(|_| println!("{}", "Master password updated successfully".green()));
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::{cli::io::MockPromptPassword, pass::PasswordEntry};

    use super::*;
    use passwords::PasswordGenerator;
    use rstest::rstest;
    use tempfile::NamedTempFile;

    #[rstest]
    #[case("service1".to_string(), Some("username1".to_string()), Some("password1"), false)]
    #[case("service2".to_string(), None, Some("password2"), false)]
    #[case("service3".to_string(), Some("username3".to_string()), None, true)]
    fn test_add_password(
        #[case] service: String,
        #[case] username: Option<String>,
        #[case] password: Option<&str>,
        #[case] generate: bool,
    ) {
        let password_generator = PasswordGenerator::new()
            .length(10)
            .lowercase_letters(true)
            .uppercase_letters(true)
            .numbers(true)
            .symbols(true)
            .strict(true);
        let master = "master_password".to_string();
        let temp_file = NamedTempFile::new().unwrap().path().to_path_buf();
        let output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        let mut password_store = PasswordStore::new(temp_file, master).unwrap();
        let mock_prompt_password = &MockPromptPassword::new();
        let result = add_password(
            &mut writer,
            mock_prompt_password,
            &mut password_store,
            service.clone(),
            username.clone(),
            password.map(|s| s.to_string()),
            generate,
            password_generator,
        );
        assert!(result.is_ok());
        assert!(password_store.find(service, username).is_some());
    }

    #[rstest]
    #[case(Length::Eight, true, true, true, true, 2)]
    #[case(Length::Sixteen, false, true, true, true, 2)]
    #[case(Length::ThirtyTwo, true, false, true, true, 3)]
    #[case(Length::Sixteen, true, false, false, false, 2)]
    #[case(Length::ThirtyTwo, false, true, false, false, 3)]
    fn test_generate_password(
        #[case] length: Length,
        #[case] symbols: bool,
        #[case] uppercase: bool,
        #[case] lowercase: bool,
        #[case] numbers: bool,
        #[case] count: usize,
    ) {
        let mut output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        generate_password(
            length,
            symbols,
            uppercase,
            lowercase,
            numbers,
            count,
            &mut writer,
        )
        .unwrap();
        output = writer.into_inner();
        let output_str = String::from_utf8(output).unwrap();
        println!("{}", output_str);
        let lines: Vec<&str> = output_str.trim().lines().collect();
        assert_eq!(lines.len(), count);
        if count == 1 {
            println!("{}", output_str);
            assert!(output_str.contains("Random password generated"))
        }
    }

    #[test]
    fn test_generate_password_all_false() {
        let mut output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        generate_password(Length::Eight, false, false, false, false, 1, &mut writer).unwrap();
        output = writer.into_inner();
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains(
            "Error generating password: You need to enable at least one kind of characters."
        ))
    }

    #[rstest(
        service, username, password, expect_password_found,
        case("service1".to_string(), Some("username1".to_string()), "password1".to_string(), true),
        case("service2", None, "password2".to_string(), true),
        case("service3", None, "password3".to_string(), false)
    )]
    fn test_show_password(
        service: String,
        username: Option<String>,
        password: String,
        expect_password_found: bool,
    ) {
        let master = "master_password".to_string();
        let temp_file = NamedTempFile::new().unwrap().path().to_path_buf();
        let mut password_store = PasswordStore::new(temp_file, master).unwrap();
        let output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        let mock_prompt_password = &MockPromptPassword::new();
        add_password(
            &mut writer,
            mock_prompt_password,
            &mut password_store,
            service.clone(),
            username.clone(),
            Some(password.clone()),
            false,
            PasswordGenerator::default(),
        )
        .unwrap();

        let mut output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        let result = if expect_password_found {
            show_password(&mut password_store, service, username, &mut writer)
        } else {
            show_password(
                &mut password_store,
                "not_found_service".to_string(),
                Some("not_found_username".to_string()),
                &mut writer,
            )
        };
        assert!(result.is_ok());

        output = writer.into_inner();
        let output_str = String::from_utf8(output).unwrap();
        if expect_password_found {
            assert!(output_str.contains(&password));
        } else {
            assert!(output_str.contains("Password not found"));
        }
    }

    #[rstest(
        show_passwords,
        passwords,
        case(false, vec![]),
        case(false, vec![("service1", "username1", "password1")]),
        case(true, vec![("service1", "username1", "password1")]),
        case(false, vec![("service1", "username1", "password1"), ("service2", "username2", "password2")]),
        case(true, vec![("service1", "username1", "password1"), ("service2", "username2", "password2")]),
        case(false, vec![("service1", "username1", "password1"), ("service2", "username2", "password2"), ("service3", "username3", "password3")]),
        case(true, vec![("service1", "username1", "password1"), ("service2", "username2", "password2"), ("service3", "username3", "password3")])
    )]
    fn test_list_passwords(show_passwords: bool, passwords: Vec<(&str, &str, &str)>) {
        let master = "master_password".to_string();
        let temp_file = NamedTempFile::new().unwrap().path().to_path_buf();
        let mut password_store = PasswordStore::new(temp_file, master).unwrap();
        let output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        let mock_prompt_password = &MockPromptPassword::new();

        for (service, username, password) in passwords.iter() {
            add_password(
                &mut writer,
                mock_prompt_password,
                &mut password_store,
                service.to_string(),
                Some(username.to_string()),
                Some(password.to_string()),
                false,
                PasswordGenerator::default(),
            )
            .unwrap();
        }

        let mut output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        let result = list_passwords(&mut password_store, show_passwords, &mut writer);
        assert!(result.is_ok());

        output = writer.into_inner();
        let output_str = String::from_utf8(output).unwrap();

        if passwords.is_empty() {
            assert!(output_str.contains(&"No passwords found!".yellow().to_string()));
        }

        for (service, username, password) in passwords.iter() {
            if show_passwords {
                assert!(output_str.contains(&format!(
                    "Service: {}, Username: {}, Password: {}",
                    service.blue(),
                    username.blue(),
                    password.blue()
                )))
            } else {
                assert!(output_str.contains(&format!(
                    "Service: {}, Username: {}, Password: {}",
                    service.blue(),
                    username.blue(),
                    "***".blue()
                )))
            }
        }
    }

    #[rstest(
    passwords_to_add,
    password_to_remove,
    expected_passwords,
    case(
        vec![("service1", "username1", "password1")],
        ("service1", "username1"),
        vec![]
    ),
    case(
        vec![("service1", "username1", "password1"), ("service2", "username2", "password2")],
        ("service1", "username1"),
        vec![("service2", "username2", "password2")]
    ),
    case(
        vec![("service1", "username1", "password1"), ("service2", "username2", "password2"), ("service3", "username3", "password3")],
        ("service1", "username1"),
        vec![("service2", "username2", "password2"), ("service3", "username3", "password3")]
    ),
    case(
        vec![("service1", "username1", "password1"), ("service2", "username2", "password2"), ("service3", "username3", "password3")],
        ("service5", "username5"),
        vec![("service1", "username1", "password1"), ("service2", "username2", "password2"), ("service3", "username3", "password3")]
    )
    )]
    fn test_remove_password(
        passwords_to_add: Vec<(&str, &str, &str)>,
        password_to_remove: (&str, &str),
        expected_passwords: Vec<(&str, &str, &str)>,
    ) {
        let master = "master_password".to_string();
        let temp_file = NamedTempFile::new().unwrap().path().to_path_buf();
        let mut password_store = PasswordStore::new(temp_file, master).unwrap();
        let output = Vec::new();
        let mut writer = std::io::Cursor::new(output);
        let mock_prompt_password = &MockPromptPassword::new();

        for (service, username, password) in passwords_to_add.iter() {
            add_password(
                &mut writer,
                mock_prompt_password,
                &mut password_store,
                service.to_string(),
                Some(username.to_string()),
                Some(password.to_string()),
                false,
                PasswordGenerator::default(),
            )
            .unwrap();
        }

        let (service, username) = password_to_remove;
        let mut output = Vec::new();
        let result = remove_password(
            &mut output,
            &mut password_store,
            service.to_string(),
            Some(username.to_string()),
        );
        assert!(result.is_ok());

        for (service, username, password) in expected_passwords.iter() {
            assert_eq!(
                password_store.find(service.to_string(), Some(username.to_string())),
                Some(&PasswordEntry::new(
                    service.to_string(),
                    Some(username.to_string()),
                    password.to_string()
                ))
            );
        }
    }

    #[test]
    fn test_update_master_password() {
        let temp_file = NamedTempFile::new().unwrap().path().to_path_buf();
        let mut output = Vec::new();
        let mut password_store = PasswordStore::new(temp_file, "master".to_string()).unwrap();
        update_master_password(
            &mut output,
            "new_master_password".to_string(),
            &mut password_store,
        )
        .unwrap();
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains(&"Master password updated successfully".green().to_string()));
    }
}
