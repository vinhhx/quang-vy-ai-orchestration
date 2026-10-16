use agent_brain::app;
use agent_brain::error::{BrainError, EXIT_ROUTING};
use agent_brain::projects;
use agent_brain::roles;
use agent_brain::runner;
use agent_brain::term;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "agent-brain", version, about = "QuangVy Orchestration CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Quản lý danh sách dự án
    Projects {
        #[command(subcommand)]
        subcommand: ProjectsCommands,
    },
    /// Chạy tác vụ theo vai trò (lead, dev, qa)
    Run {
        /// Vai trò thực thi
        role: roles::Role,
        /// Tên, id hoặc alias dự án
        project: String,
        /// Yêu cầu cần làm
        request: Option<String>,
        /// Slug tên task
        #[arg(long)]
        name: Option<String>,
        /// Chạy thử không sinh tác vụ
        #[arg(long)]
        dry_run: bool,
        /// In chi tiết các bước thực hiện
        #[arg(long, short)]
        verbose: bool,
    },
    /// Trạng thái nhanh của dự án
    Status {
        /// Tên, id hoặc alias dự án
        project: String,
    },
    /// Xem xét nhanh thay đổi
    Inspect {
        /// Tên, id hoặc alias dự án
        project: String,
        /// In chi tiết
        #[arg(long, short)]
        verbose: bool,
    },
    /// Dừng tiến trình của task
    Kill {
        /// ID của task
        task_id: i64,
    },
    /// Thống kê token và chi phí
    Stats {
        /// Lọc theo profile
        #[arg(long)]
        profile: Option<String>,
        /// Khoảng thời gian
        #[arg(long)]
        since: Option<String>,
        /// Xuất định dạng JSON
        #[arg(long)]
        json: bool,
    },
    /// Kiểm soát quyền hạn chế của công cụ
    Guard {
        /// Đường dẫn tới file chính sách
        #[arg(long)]
        policy: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum ProjectsCommands {
    /// Quét thư mục tìm dự án
    Scan {
        /// Thư mục gốc cần quét
        #[arg(long = "root", value_name = "DIR")]
        roots: Vec<String>,
    },
    /// Thêm dự án vào map
    Add {
        /// Đường dẫn thư mục dự án
        path: String,
        /// Loại dự án (code|media)
        #[arg(long = "type", value_name = "code|media")]
        r#type: Option<String>,
    },
}

fn main() {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    print!("{e}");
                    std::process::exit(0);
                }
                _ => {
                    eprint!("{e}");
                    std::process::exit(1);
                }
            }
        }
    };

    let cmd = match cli.command {
        Some(c) => c,
        None => {
            use clap::CommandFactory;
            let mut c = Cli::command();
            let help = c.render_help();
            eprintln!("{help}");
            std::process::exit(1);
        }
    };

    if let Commands::Guard { .. } = cmd {
        term::emit("agent-brain guard: chưa có trong bản này — chặn (fail-closed)");
        std::process::exit(2);
    }

    let mut ctx = match app::open_ctx() {
        Ok(ctx) => ctx,
        Err(e) => {
            if e.code == EXIT_ROUTING {
                term::emit_out(&e.message);
            } else {
                term::emit(&e.message);
            }
            std::process::exit(e.code);
        }
    };

    let result = match cmd {
        Commands::Guard { .. } => unreachable!(),
        Commands::Projects { subcommand } => match subcommand {
            ProjectsCommands::Scan { roots } => projects::cmd_scan(&roots),
            ProjectsCommands::Add { path, r#type } => projects::cmd_add(&path, r#type.as_deref()),
        },
        Commands::Run {
            role,
            project,
            request,
            name,
            dry_run,
            verbose,
        } => roles::cmd_run(
            &mut ctx,
            role,
            &project,
            request.as_deref(),
            name.as_deref(),
            dry_run,
            verbose,
        ),
        Commands::Status { .. } => Err(BrainError::not_available("status")),
        Commands::Inspect { .. } => Err(BrainError::not_available("inspect")),
        Commands::Kill { task_id } => runner::cmd_kill(&mut ctx, task_id),
        Commands::Stats { .. } => Err(BrainError::not_available("stats")),
    };

    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            if e.code == EXIT_ROUTING {
                term::emit_out(&e.message);
            } else {
                term::emit(&e.message);
            }
            std::process::exit(e.code);
        }
    }
}
