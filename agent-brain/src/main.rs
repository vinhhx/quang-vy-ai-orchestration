use agent_brain::app;
use agent_brain::error::EXIT_ROUTING;
use agent_brain::fastpath;
use agent_brain::guard;
use agent_brain::projects;
use agent_brain::roles;
use agent_brain::runner;
use agent_brain::stats;
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
    /// Chạy tác vụ theo vai trò (lead, dev, qa, plan-review, code-review)
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
        /// Vòng lặp (ẩn)
        #[arg(long, hide = true)]
        round: Option<i64>,
        /// Chế độ sửa theo báo cáo (qa | code-review)
        #[arg(long, value_enum)]
        fix: Option<roles::FixInput>,
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
    /// Mở dashboard web (chỉ 127.0.0.1, có token)
    Serve {
        /// Cổng (mặc định: khóa dashboard_port hoặc 7437)
        #[arg(long)]
        port: Option<u16>,
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

    if let Commands::Guard { policy } = &cmd {
        let code = guard::cmd_guard(policy.as_deref());
        std::process::exit(code);
    }

    if let Commands::Serve { port } = &cmd {
        std::process::exit(agent_brain::dashboard::cmd_serve(*port));
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
        Commands::Serve { .. } => unreachable!(),
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
            round,
            fix,
        } => {
            let opts = roles::RunOpts {
                name,
                dry_run,
                verbose,
                round,
                fix,
            };
            roles::cmd_run(&mut ctx, role, &project, request.as_deref(), &opts)
        }
        Commands::Status { project } => fastpath::cmd_status(&mut ctx, &project),
        Commands::Inspect { project, verbose } => {
            fastpath::cmd_inspect(&mut ctx, &project, verbose)
        }
        Commands::Kill { task_id } => runner::cmd_kill(&mut ctx, task_id),
        Commands::Stats {
            profile,
            since,
            json,
        } => stats::cmd_stats(&mut ctx, profile.as_deref(), since.as_deref(), json),
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
