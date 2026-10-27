use agent_brain::app;
use agent_brain::autopilot;
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
        /// Lead sửa plan theo .plan-review.md
        #[arg(long)]
        review: bool,
        /// Ghi chú cho Lead (dùng --note=NỘI DUNG)
        #[arg(long, allow_hyphen_values = true)]
        note: Option<String>,
        /// Quy trình (workflow) cho task mới
        #[arg(long)]
        workflow: Option<String>,
        /// Đội hình cho task mới (chỉ với vai trò lead)
        #[arg(long)]
        team: Option<String>,
        /// Nguồn của task (nội bộ: bot Telegram)
        #[arg(long, hide = true, value_parser = ["telegram"])]
        origin: Option<String>,
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
    /// Autopilot: chạy các bước của task theo quy trình, dừng ở bước duyệt plan
    Auto {
        /// ID task; với --start: <dự án> <yêu cầu> (đặt sau --)
        #[arg(num_args = 1..)]
        args: Vec<String>,
        /// Tạo task mới bằng bước Lead đầu tiên rồi chạy tiếp
        #[arg(long)]
        start: bool,
        /// Slug tên task (chỉ với --start)
        #[arg(long)]
        name: Option<String>,
        /// Quy trình (workflow) cho task mới (chỉ với --start)
        #[arg(long)]
        workflow: Option<String>,
        /// Chạy thêm 1 vòng sau khi autopilot đã dừng
        #[arg(long)]
        more_round: bool,
        /// Lập lại plan (Trả về Lead)
        #[arg(long)]
        replan: bool,
        /// Ghi chú cho Lead khi lập lại plan (dùng --note=NỘI DUNG)
        #[arg(long, allow_hyphen_values = true)]
        note: Option<String>,
        /// Nguồn của task (nội bộ: bot Telegram)
        #[arg(long, hide = true, value_parser = ["telegram"])]
        origin: Option<String>,
    },
    /// Duyệt plan hiện tại của task (autopilot chạy tiếp nếu quy trình bật auto)
    Approve {
        /// ID của task
        task_id: i64,
        /// Chỉ ghi duyệt, không chạy tiếp autopilot
        #[arg(long)]
        no_continue: bool,
    },
    /// Mở dashboard web (chỉ 127.0.0.1, có token)
    Serve {
        /// Cổng (mặc định: khóa dashboard_port hoặc 7437)
        #[arg(long)]
        port: Option<u16>,
    },
    /// Telegram: ghép nối bot và gửi tin thử (thông báo + nút duyệt/dừng)
    Telegram {
        #[command(subcommand)]
        subcommand: TelegramCommands,
    },
}

#[derive(Subcommand, Debug)]
enum TelegramCommands {
    /// Ghép nối: nhập token bot (không hiện trên màn hình), gửi mã 6 số cho bot
    Setup,
    /// Gửi một tin thử tới chat đã ghép nối
    Test,
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

    if let Commands::Telegram { subcommand } = &cmd {
        std::process::exit(match subcommand {
            TelegramCommands::Setup => agent_brain::telegram::setup::cmd_setup(),
            TelegramCommands::Test => agent_brain::telegram::setup::cmd_test(),
        });
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
        Commands::Telegram { .. } => unreachable!(),
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
            review,
            note,
            workflow,
            team,
            origin,
        } => {
            let opts = roles::RunOpts {
                name,
                dry_run,
                verbose,
                round,
                fix,
                review,
                note,
                workflow,
                origin,
                team,
            };
            roles::cmd_run(&mut ctx, role, &project, request.as_deref(), &opts)
        }
        Commands::Status { project } => fastpath::cmd_status(&mut ctx, &project),
        Commands::Inspect { project, verbose } => {
            fastpath::cmd_inspect(&mut ctx, &project, verbose)
        }
        Commands::Kill { task_id } => runner::cmd_kill(&mut ctx, task_id),
        Commands::Auto {
            args,
            start,
            name,
            workflow,
            more_round,
            replan,
            note,
            origin,
        } => {
            let opts = autopilot::AutoOpts {
                start,
                args,
                name,
                workflow,
                more_round,
                replan,
                note,
                origin,
            };
            autopilot::cmd_auto(&mut ctx, &opts)
        }
        Commands::Approve {
            task_id,
            no_continue,
        } => autopilot::cmd_approve(&mut ctx, task_id, no_continue),
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
