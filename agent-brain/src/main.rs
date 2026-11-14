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
        subcommand: Option<ProjectsCommands>,
    },
    /// Chạy tác vụ theo vai trò (lead, dev, qa, plan-review, code-review, analyst)
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
        /// Lead sửa plan theo báo cáo review plan
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
        /// Mức độ khó cho task (de | vua | kho)
        #[arg(long)]
        level: Option<String>,
        /// Nguồn của task (nội bộ: bot Telegram)
        #[arg(long, hide = true, value_parser = ["telegram"])]
        origin: Option<String>,
        /// Lượt chốt của Dev sau khi hết giờ (nội bộ: autopilot)
        #[arg(long, hide = true)]
        finalize: bool,
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
        /// Đội hình cho task mới (chỉ với --start)
        #[arg(long)]
        team: Option<String>,
        /// Mức độ khó cho task mới (chỉ với --start: de | vua | kho)
        #[arg(long)]
        level: Option<String>,
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
    /// Quét run/task bị treo, driver mồ côi, hàng chờ kẹt, ổ đĩa đầy; chỉ cảnh báo (tự sửa chỉ run chết)
    Doctor {
        /// In mảng JSON {kind,key,task_id,run_id,text}
        #[arg(long)]
        json: bool,
        /// Gửi tin Telegram tóm tắt các phát hiện mới (không gửi lặp trong realert_minutes)
        #[arg(long)]
        notify: bool,
        /// Đánh dấu lost các run đang ghi "running" mà tiến trình đã chết (tự sửa duy nhất)
        #[arg(long)]
        heal: bool,
        /// Kiểm tra thêm agent-brain serve còn chạy khi vẫn có việc đang chạy
        #[arg(long)]
        check_serve: bool,
    },
    /// Telegram: ghép nối bot và gửi tin thử (thông báo + nút duyệt/dừng)
    Telegram {
        #[command(subcommand)]
        subcommand: TelegramCommands,
    },
    /// Bước Dev do nhân sự làm (engine người): xong / trả lại
    Human {
        #[command(subcommand)]
        action: HumanCommands,
    },
    /// Xem prompt sau khi ghép ghi chú nghiệp vụ
    Prompt {
        #[command(subcommand)]
        subcommand: PromptCommands,
    },
    /// Quản lý bug từ Trello và Google Sheet
    Fixbug {
        #[command(subcommand)]
        subcommand: FixbugCommands,
    },
    /// Nhật ký công việc sang vault Obsidian
    Journal {
        #[command(subcommand)]
        subcommand: JournalCommands,
    },
    /// Quản lý phòng ban và thành viên
    Dept {
        #[command(subcommand)]
        subcommand: DeptCommands,
    },
    /// Xem container docker đang chạy mà lệnh php/composer của dự án được chuyển vào
    Container {
        /// Tên, id hoặc alias dự án
        project: String,
    },
    /// Worktree và nhánh của task (dự án ở chế độ worktree)
    Task {
        #[command(subcommand)]
        subcommand: TaskCommands,
    },
    /// Quản lý tách việc thành task con
    Split {
        #[command(subcommand)]
        subcommand: SplitCommands,
    },
}

#[derive(Subcommand, Debug)]
enum SplitCommands {
    /// In bảng danh sách việc con (idx, tiêu đề, state, task con, outcome)
    Status {
        /// ID của task cha
        task_id: i64,
    },
    /// Tổng hợp kết quả các task con thành báo cáo gộp
    Aggregate {
        /// ID của task cha
        task_id: i64,
    },
    /// Chạy lại các việc con đã failed hoặc skipped (chưa có task con)
    Retry {
        /// ID của task cha
        task_id: i64,
    },
}

#[derive(Subcommand, Debug)]
enum TaskCommands {
    /// Dọn worktree của task đã xong (git worktree remove, không --force, không xoá nhánh)
    Cleanup {
        /// ID của task
        task_id: Option<i64>,
        /// Dọn mọi worktree có nhánh đã merge vào nhánh gốc
        #[arg(long)]
        merged: bool,
        /// Xác nhận dọn dù nhánh chưa merge
        #[arg(long)]
        yes: bool,
        /// Với --merged: sau khi dọn, xoá luôn các nhánh task đã merge (git branch -d, không bao giờ -D)
        #[arg(long)]
        branches: bool,
    },
    /// Merge nhánh task (đã qua QA) vào nhánh gốc trên repo chính (git merge --no-ff; --push: fetch, merge, push không force, xoá nhánh task)
    Merge {
        /// ID của task
        task_id: i64,
        /// Merge & push: lấy origin/<nhánh gốc>, checkout nhánh gốc, merge, push lên origin (không force), xoá nhánh task (git branch -d)
        #[arg(long)]
        push: bool,
    },
}

#[derive(Subcommand, Debug)]
enum DeptCommands {
    /// Danh sách phòng ban và thành viên
    List,
    /// Kiểm tra tính hợp lệ của departments.json
    Check,
    /// Chạy thử thuật toán chọn thành viên cho vai trò của task
    Pick {
        /// ID của task
        task_id: i64,
        /// Vai trò (lead, dev, qa, analyst, ...)
        role: String,
    },
}

#[derive(Subcommand, Debug)]
enum FixbugCommands {
    /// Kiểm tra cấu hình và kết nối tới nguồn bug
    Check {
        /// Tên, id hoặc alias dự án
        project: String,
    },
    /// Lấy bug từ nguồn vào hàng chờ intake
    Pull {
        /// Tên, id hoặc alias dự án
        project: String,
        /// Số bug tối đa lấy mỗi lần (mặc định: 10)
        #[arg(long)]
        max: Option<usize>,
        /// Bỏ qua kiểm tra "chưa nhập trạng thái skill /fix-bugs cũ" (không bỏ qua .lock)
        #[arg(long)]
        skip_legacy_check: bool,
    },
    /// Nhập trạng thái các bug của skill /fix-bugs cũ (chỉ đọc tasks/*/state.json)
    ImportLegacy {
        /// Tên, id hoặc alias dự án
        project: String,
        /// Bug cũ need_info/plan_ready: skip = ghi 'pulled' (mặc định), take = để pull kéo thành task mới
        #[arg(long, default_value = "skip")]
        open: agent_brain::fixbug::legacy::OpenMode,
        /// Chỉ in bảng KEY | state cũ | sẽ ghi, không ghi gì
        #[arg(long)]
        dry_run: bool,
    },
    /// Danh sách bug đã lấy trong dự án
    List {
        /// Tên, id hoặc alias dự án
        project: String,
    },
    /// Đánh dấu bug đã sửa và deploy (chuyển cột Testing trên Trello)
    Deployed {
        /// Tên, id hoặc alias dự án
        project: String,
        /// Mã các bug cần đánh dấu (ví dụ: T-42 S-1)
        #[arg(required = true)]
        keys: Vec<String>,
        /// Ghi chú kèm theo
        #[arg(long)]
        note: Option<String>,
    },
    /// Từ chối sửa bug (chuyển cột Reject trên Trello)
    Reject {
        /// Tên, id hoặc alias dự án
        project: String,
        /// Mã các bug cần từ chối (ví dụ: T-42 S-1)
        #[arg(required = true)]
        keys: Vec<String>,
        /// Lý do từ chối (bắt buộc)
        #[arg(long, required = true)]
        reason: String,
    },
    /// Khởi tạo cấu hình fixbugs.json trong thư mục tri thức của dự án
    Init {
        /// Tên, id hoặc alias dự án
        project: String,
        /// Chỉ in cấu hình đề xuất, không ghi file
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum JournalCommands {
    /// Dựng lại toàn bộ nhật ký từ brain.db, commit và đẩy lên repo vault (--no-push: chỉ ghi vào bản clone để xem diff)
    Export {
        #[arg(long)]
        no_push: bool,
        /// Cho phép một lần xuất xóa nhiều tệp nhật ký đang có (bỏ qua chốt chặn xóa hàng loạt)
        #[arg(long)]
        allow_removals: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PromptCommands {
    /// In prompt cuối cùng ra stdout (không chạy gì)
    Show {
        /// Tên, id hoặc alias dự án
        #[arg(long)]
        project: String,
        /// Vai trò (lead, dev, qa, plan-review, code-review)
        #[arg(long)]
        role: String,
        /// Quy trình (workflow)
        #[arg(long)]
        workflow: Option<String>,
        /// Đội hình
        #[arg(long)]
        team: Option<String>,
        /// Thành viên phòng ban
        #[arg(long)]
        member: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum HumanCommands {
    /// Lấy nhánh nhân sự đã đẩy và chạy tiếp quy trình
    Done {
        /// ID của task
        task_id: i64,
    },
    /// Trả việc lại cho nhân sự kèm báo cáo
    Return {
        /// ID của task
        task_id: i64,
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
    /// Danh sách dự án
    List,
    /// Quét thư mục tìm dự án
    Scan {
        /// Thư mục gốc cần quét
        #[arg(long = "root", value_name = "DIR")]
        roots: Vec<String>,
        /// Chỉ liệt kê repo chưa có trong map, không ghi gì
        #[arg(long = "list")]
        list: bool,
    },
    /// Thêm dự án vào map
    Add {
        /// Đường dẫn thư mục dự án
        path: String,
        /// Loại dự án (code|media|idea)
        #[arg(long = "type", value_name = "code|media|idea")]
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

    // Before `open_ctx`: that call marks dead runs `lost`, which doctor must only do with --heal.
    if let Commands::Doctor {
        json,
        notify,
        heal,
        check_serve,
    } = &cmd
    {
        std::process::exit(agent_brain::doctor::cmd_doctor(*json, *notify, *heal, *check_serve));
    }

    if let Commands::Telegram { subcommand } = &cmd {
        std::process::exit(match subcommand {
            TelegramCommands::Setup => agent_brain::telegram::setup::cmd_setup(),
            TelegramCommands::Test => agent_brain::telegram::setup::cmd_test(),
        });
    }

    if let Commands::Container { project } = &cmd {
        let code = match agent_brain::container::cmd_container(project) {
            Ok(code) => code,
            Err(e) => {
                if e.code == EXIT_ROUTING {
                    term::emit_out(&e.message);
                } else {
                    term::emit(&e.message);
                }
                e.code
            }
        };
        std::process::exit(code);
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
        Commands::Doctor { .. } => unreachable!(),
        Commands::Container { .. } => unreachable!(),
        Commands::Human { action } => match action {
            HumanCommands::Done { task_id } => agent_brain::human::cmd_human(
                &mut ctx,
                agent_brain::human::HumanAction::Done,
                task_id,
            ),
            HumanCommands::Return { task_id } => agent_brain::human::cmd_human(
                &mut ctx,
                agent_brain::human::HumanAction::Return,
                task_id,
            ),
        },
        Commands::Projects { subcommand } => match subcommand {
            None | Some(ProjectsCommands::List) => projects::cmd_list(),
            Some(ProjectsCommands::Scan { roots, list }) => projects::cmd_scan(&roots, list),
            Some(ProjectsCommands::Add { path, r#type }) => projects::cmd_add(&path, r#type.as_deref()),
        },
        Commands::Prompt { subcommand } => match subcommand {
            PromptCommands::Show {
                project,
                role,
                workflow,
                team,
                member,
            } => agent_brain::prompts::cmd_show(
                &ctx,
                &project,
                &role,
                workflow.as_deref(),
                team.as_deref(),
                member.as_deref(),
            ),
        },
        Commands::Fixbug { subcommand } => Ok(match subcommand {
            FixbugCommands::Check { project } => agent_brain::fixbug::cmd_check(&ctx, &project),
            FixbugCommands::Pull {
                project,
                max,
                skip_legacy_check,
            } => agent_brain::fixbug::cmd_pull(&mut ctx, &project, max, skip_legacy_check),
            FixbugCommands::ImportLegacy {
                project,
                open,
                dry_run,
            } => agent_brain::fixbug::cmd_import_legacy(&mut ctx, &project, open, dry_run),
            FixbugCommands::List { project } => agent_brain::fixbug::cmd_list(&ctx, &project),
            FixbugCommands::Deployed {
                project,
                keys,
                note,
            } => agent_brain::fixbug::cmd_deployed(&mut ctx, &project, &keys, note.as_deref()),
            FixbugCommands::Reject {
                project,
                keys,
                reason,
            } => agent_brain::fixbug::cmd_reject(&mut ctx, &project, &keys, &reason),
            FixbugCommands::Init { project, dry_run } => {
                agent_brain::fixbug::cmd_init(&ctx, &project, dry_run)
            }
        }),
        Commands::Journal { subcommand } => match subcommand {
            JournalCommands::Export {
                no_push,
                allow_removals,
            } => agent_brain::journal::cmd_export(&ctx, no_push, allow_removals),
        },
        Commands::Task { subcommand } => match subcommand {
            TaskCommands::Cleanup {
                task_id,
                merged,
                yes,
                branches,
            } => agent_brain::workspace::cmd_cleanup(&mut ctx, task_id, merged, yes, branches),
            TaskCommands::Merge { task_id, push: false } => {
                agent_brain::workspace::cmd_merge(&mut ctx, task_id)
            }
            TaskCommands::Merge { task_id, push: true } => {
                agent_brain::workspace::cmd_merge_push(&mut ctx, task_id)
            }
        },
        Commands::Split { subcommand } => match subcommand {
            SplitCommands::Status { task_id } => agent_brain::split::cmd_status(&ctx, task_id),
            SplitCommands::Aggregate { task_id } => {
                agent_brain::split::cmd_aggregate(&ctx, task_id)
            }
            SplitCommands::Retry { task_id } => agent_brain::split::cmd_retry(&ctx, task_id),
        },
        Commands::Dept { subcommand } => match subcommand {
            DeptCommands::List => agent_brain::departments::cmd_list(&ctx),
            DeptCommands::Check => agent_brain::departments::cmd_check(&ctx),
            DeptCommands::Pick { task_id, role } => {
                agent_brain::departments::cmd_pick(&ctx, task_id, &role)
            }
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
            level,
            origin,
            finalize,
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
                level,
                finalize,
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
            team,
            level,
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
                team,
                level,
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
