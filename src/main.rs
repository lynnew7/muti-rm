use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{remove_dir, remove_file};
use std::path::Path;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// 要删除的目标路径
    path: String,
    /// 线程数 (默认为CPU核心数)
    #[arg(short, long, default_value_t = num_cpus::get())]
    threads: usize,
}

fn main() {
    let args = Args::parse();
    let target = Path::new(&args.path);

    // 收集所有文件和目录
    let (files, mut dirs): (Vec<_>, Vec<_>) = WalkDir::new(target)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().exists()) // 过滤已删除项
        .partition(|e| e.file_type().is_file());

    let total_files = files.len();
    // 进度条初始化
    let pb = ProgressBar::new(total_files as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    let counter = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = crossbeam_channel::bounded(1000);
    let pb = Arc::new(pb);

    // 创建工作线程
    for _ in 0..args.threads {
        let rx = rx.clone();
        let counter = counter.clone();
        let pb = pb.clone();
        thread::spawn(move || {
            for path in rx {
                if let Err(e) = remove_file(path) {
                    eprintln!("删除失败: {}", e);
                }
                counter.fetch_add(1, Ordering::Relaxed);
                pb.inc(1);
            }
        });
    }

    // 发送文件路径到通道
    for file in files {
        let _ = tx.send(file.into_path());
    }

    // 等待所有文件删除完成
    drop(tx);
    while counter.load(Ordering::Relaxed) < total_files {
        thread::sleep(std::time::Duration::from_millis(100));
    }
    pb.finish_with_message("文件删除完成");

    // 删除目录（按深度逆序）
    dirs.sort_by_key(|d| std::cmp::Reverse(d.depth()));
    for dir in dirs {
        if let Err(e) = remove_dir(dir.path()) {
            eprintln!("目录删除失败: {}", e);
        }
    }

    println!("操作完成，共删除 {} 个文件", total_files);
}
