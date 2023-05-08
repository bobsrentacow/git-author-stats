use chrono::{Datelike, Local};
use hashbrown::HashMap;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::channel;
use threadpool::ThreadPool;

mod cli_args;
use cli_args::Args;
use clap::Parser;

type Author = String;
type Date = String;
type Count = i32;
type AuthorCount = HashMap<Author, Count>;
type AuthorPerformance = HashMap<Date, AuthorCount>;

fn git_repo_root(path: &str) -> Option<String> {
    let repo_root_out =
        Command::new("git")
                .arg("rev-parse")
                .arg("--show-toplevel")
                .current_dir(path)
                .output()
                .expect("git rev-parse failed to start");
    match repo_root_out.status.success() {
        false => return None,
        true => {
            let repo_root = String::from_utf8_lossy(&repo_root_out.stdout);
            return Some(repo_root.trim().to_string());
        },
    }
}

fn git_revision(repo_root: &str, branch: &Option<String>, date: &Option<String>) -> Option<String> {
    //println!("repo_root: {repo_root}");
    //println!("branch: {branch:?}");
    //println!("date: {date:?}");
    // git log --format=format:"%H" --before=2023-01-01
    let mut cmd = Command::new("git");
    cmd.arg("log");
    cmd.arg("-n1").arg("--format=format:%H");
    if let Some(date) = date {
        cmd.arg(format!("--before={date}"));
    }
    if let Some(branch) = branch {
       cmd.arg(branch);
    }
    cmd.current_dir(repo_root);
    //println!("cmd: {:?}", cmd);
    let cmd_out = cmd.output().expect("git log failed to start");
    //println!("{:?}", cmd_out.status);
    //println!("{:?}", cmd_out.stdout);
    match cmd_out.status.success() {
        false => return None,
        true => {
            let revision = String::from_utf8_lossy(&cmd_out.stdout);
            return Some(revision.trim().to_string());
        },
    }
}

fn git_files(repo_root: &str, revision: &str) -> Vec<String> {
    let ls_tree_out =
        Command::new("git")
                .arg("ls-tree")
                .arg("-r")
                .arg(revision)
                .arg("--name-only")
                .current_dir(repo_root)
                .output()
                .expect("git ls-tree failed to start");
    return String::from_utf8_lossy(&ls_tree_out.stdout)
        .lines()
        .map(|x| x.to_string())
        .collect();
}

fn reason_to_skip(path_buf: &PathBuf) -> Option<String> {
    // List of file extensions to skip
    let binary_ext_list = [
        "bin",
        "data",
        "elf",
        "gz",
        "hex128",
        "hex8",
        "pdf",
        "png",
        "tar",
        "wcfg",
        "xlsx",
    ];

    let generated_ext_list = [
        "v",
        "xml",
        "edif",
        "edf",
        "rpt",
        "xci",
    ];

    let path = path_buf.to_str().unwrap();
    if path.starts_with("xip/") {
        return Some("mostly imported     ".to_string());
    }
    if path.starts_with("cache/") {
        return Some("generated           ".to_string());
    }

    if let Some(ext) = path_buf.extension() {
        let ext = ext.to_str().unwrap();
        if binary_ext_list.contains(&ext) {
            return Some("binary extension    ".to_string());
        }
        if generated_ext_list.contains(&ext) {
            return Some("autogenerated       ".to_string());
        }
    }

    if let Some(name) = path_buf.file_name() {
        let name = name.to_str().unwrap();
        if name.ends_with(".bd.tcl") {
            return Some("mostly autogenerated".to_string());
        }
    }

    return None;
}

fn git_author_line_count(repo_root: &str, revision: &str, file_path: &str) -> AuthorCount {
    let mut authors = AuthorCount::new();

    let blame_out =
        Command::new("git")
                .arg("blame")
                .arg("--line-porcelain")
                .arg(revision)
                .arg(file_path)
                .current_dir(repo_root)
                .output()
                .expect("git blame failed to start");
    let auth_lines = String::from_utf8_lossy(&blame_out.stdout);
    auth_lines.lines().filter(|x| x.starts_with("author ")).for_each(|x| {
        let author = x[7..].to_string();
        *authors.entry_ref(&author).or_insert(0) += 1;
    });

    return authors;
}

fn reformat(perf: &AuthorPerformance) -> AuthorPerformance {
    lazy_static! {
        // Regex for reformatting author names
        static ref RE_SPECIAL: Regex = Regex::new(r"[-_\.]").unwrap();
        static ref RE_CAPITAL: Regex = Regex::new(r"\b[a-z]").unwrap();
    };

    let mut formatted = AuthorPerformance::new();

    perf.iter().for_each(|(date, acnt_in)| {
        let acnt_out = formatted.entry_ref(date).or_insert(AuthorCount::new());
        for author in acnt_in.keys() {
            let cnt_in = acnt_in[author];
            // reformat author name
            let mut author = RE_SPECIAL.replace_all(&author, " ").to_lowercase();
            for mat in RE_CAPITAL.find_iter(&author.clone()) {
                let mut c = author.chars().nth(mat.start()).unwrap();
                c = c.to_uppercase().nth(0).unwrap();
                author.replace_range(mat.start()..mat.start()+1, &c.to_string());
            }

            *acnt_out.entry_ref(&author).or_insert(0) += cnt_in;
        }
    });

    return formatted;
}

fn display_results(_opt: &Args, perf: &AuthorPerformance) { //, skip_files: i32, use_files: i32) {
    let perf = reformat(&perf);

    let mut dates = perf.keys().map(|x| x.to_string()).collect::<Vec<String>>();
    dates.sort();

    let mut authors = Vec::new();
    for date in &dates {
        if let Some(acnt) = perf.get(date) {
            authors.extend(acnt.keys().map(|x| x.to_string()));
        }
    }
    authors.sort();
    authors.dedup();
    let long_auth = authors.iter().map(|x| x.len()).max().unwrap_or(0);

    print!("{:<long_auth$}, ", "date");
    for date in &dates {
       print!("{:>10}, ", date);
    }
    println!();

    for author in authors {
        print!("{author:<long_auth$}, ");
        for date in &dates {
            if let Some(acnt) = perf.get(date) {
                print!("{:>10}, ", acnt.get(&author).unwrap_or(&0));
            }
        }
        println!();
    };
    println!();
}

fn main() {
    let opt = Args::parse();
    let repo_root = git_repo_root(&opt.path).expect("Not a git repo");

    let mut dates = Vec::new();
    let dt = Local::now();
    for year in 2016..=dt.year() {
        for month in 1..=12 {
            dates.push(format!("{year:4}-{month:02}-01"));
        }
    }

    // HashMap<date, HashMap<name, count>>
    let mut authors = AuthorPerformance::new();

    for date in dates.iter() {
        let revision = git_revision(&repo_root, &opt.branch, &Some(date.to_string())).expect("Failed to get revision from branch and date");
        let files = git_files(&repo_root, &revision);

        let files: Vec<String> = files.iter().filter(|f| {
            let pb = PathBuf::from(&f);
            if let Some(_) = reason_to_skip(&pb) {
                false
            } else {
                true
            }
        }).map(|x| x.to_string()).collect();
        if files.len() == 0 { continue; }

        let pool = ThreadPool::new(files.len().min(16)); // TODO: make this configurable, default to # of cores
        let (tx, rx) = channel();
        for f in files.iter() {
            let trepo_root = repo_root.clone();
            let trevision = revision.clone();
            let tf = f.clone();
            let ttx = tx.clone();
            pool.execute(move || {
                ttx.send(git_author_line_count(&trepo_root, &trevision, &tf)).unwrap();
            });
        };

        let mut dauth = AuthorCount::new();
        rx.iter().take(files.len()).for_each(|fauth| {
            fauth.iter().for_each(|(author, count)| {
                *dauth.entry_ref(author).or_insert(0) += count;
            });
        });

        let date_str = date.to_string();
        authors.insert(date_str, dauth);
    };

    display_results(&opt, &authors);//, skip_files, use_files);
}


