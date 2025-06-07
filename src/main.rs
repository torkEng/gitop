use anyhow::Result;
use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use git2::Repository;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState, Paragraph, Wrap},
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::time;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    repositories: Vec<RepoConfig>,
    refresh_interval: u64, // seconds
    max_commits: usize,    // number of commits to show when expanded
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepoConfig {
    name: String,
    path: String,
    remote: Option<String>, // defaults to "origin"
}

#[derive(Debug, Clone)]
struct RepoStatus {
    name: String,
    path: PathBuf,
    ahead: usize,
    behind: usize,
    last_update: Instant,
    flash_until: Option<Instant>,
    flash_color: Option<Color>,
    expanded: bool,
    recent_commits: Vec<CommitInfo>,
}

#[derive(Debug, Clone)]
struct CommitInfo {
    hash: String,
    author: String,
    message: String,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct ConsoleMessage {
    timestamp: DateTime<Utc>,
    repo: String,
    author: String,
    message: String,
}

struct App {
    repos: Arc<Mutex<Vec<RepoStatus>>>,
    console_messages: Arc<Mutex<Vec<ConsoleMessage>>>,
    table_state: TableState,
    should_quit: bool,
    max_commits: usize,
}

impl App {
    fn new(config: Config) -> Self {
        let repos: Vec<RepoStatus> = config
            .repositories
            .into_iter()
            .map(|repo_config| RepoStatus {
                name: repo_config.name,
                path: expand_path(&repo_config.path),
                ahead: 0,
                behind: 0,
                last_update: Instant::now(),
                flash_until: None,
                flash_color: None,
                expanded: false,
                recent_commits: Vec::new(),
            })
            .collect();

        let repos_empty = repos.is_empty();
        
        Self {
            repos: Arc::new(Mutex::new(repos)),
            console_messages: Arc::new(Mutex::new(Vec::new())),
            table_state: {
                let mut state = TableState::default();
                if !repos_empty {
                    state.select(Some(0)); // Start with first repository selected
                }
                state
            },
            should_quit: false,
            max_commits: config.max_commits,
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Down => self.next(),
            KeyCode::Up => self.previous(),
            KeyCode::Enter => self.toggle_expand(),
            _ => {}
        }
    }

    fn next(&mut self) {
        let repos = self.repos.lock().unwrap();
        if repos.is_empty() {
            return;
        }
        
        let current_repo_index = self.get_selected_repo_index(&repos);
        let next_repo_index = if current_repo_index >= repos.len() - 1 {
            0
        } else {
            current_repo_index + 1
        };
        
        // Calculate the table row for this repository
        let table_row = self.calculate_table_row(&repos, next_repo_index);
        self.table_state.select(Some(table_row));
    }

    fn previous(&mut self) {
        let repos = self.repos.lock().unwrap();
        if repos.is_empty() {
            return;
        }
        
        let current_repo_index = self.get_selected_repo_index(&repos);
        let prev_repo_index = if current_repo_index == 0 {
            repos.len() - 1
        } else {
            current_repo_index - 1
        };
        
        // Calculate the table row for this repository
        let table_row = self.calculate_table_row(&repos, prev_repo_index);
        self.table_state.select(Some(table_row));
    }
    
    fn get_selected_repo_index(&self, repos: &[RepoStatus]) -> usize {
        if repos.is_empty() {
            return 0;
        }
        
        if let Some(selected_table_row) = self.table_state.selected() {
            // Convert table row back to repository index
            let mut current_table_row = 0;
            for (repo_index, repo) in repos.iter().enumerate() {
                if current_table_row == selected_table_row {
                    return repo_index;
                }
                current_table_row += 1;
                if repo.expanded {
                    current_table_row += repo.recent_commits.len();
                }
                if current_table_row > selected_table_row {
                    return repo_index;
                }
            }
        }
        0
    }
    
    fn calculate_table_row(&self, repos: &[RepoStatus], repo_index: usize) -> usize {
        let mut table_row = 0;
        for (i, repo) in repos.iter().enumerate() {
            if i == repo_index {
                return table_row;
            }
            table_row += 1; // Repository row
            if repo.expanded {
                table_row += repo.recent_commits.len(); // Commit rows
            }
        }
        table_row
    }

    fn toggle_expand(&mut self) {
        let mut repos = self.repos.lock().unwrap();
        if repos.is_empty() {
            return;
        }
        
        let repo_index = self.get_selected_repo_index(&repos);
        
        if let Some(repo) = repos.get_mut(repo_index) {
            repo.expanded = !repo.expanded;
            if repo.expanded {
                // Fetch recent commits when expanding
                repo.recent_commits = get_recent_commits(&repo.path, self.max_commits);
            }
        }
        
        // Recalculate the table row after expanding/collapsing
        let table_row = self.calculate_table_row(&repos, repo_index);
        self.table_state.select(Some(table_row));
    }
}

fn expand_path(path: &str) -> PathBuf {
    if path.starts_with('~') {
        // Try HOME first (Unix/Linux), then USERPROFILE (Windows)
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let mut home_path = PathBuf::from(home);
            // Handle both "~/" and "~" cases
            if path.len() > 1 && path.chars().nth(1) == Some('/') {
                home_path.push(&path[2..]); // Skip "~/"
            } else if path.len() > 1 {
                home_path.push(&path[1..]); // Skip "~"
            }
            home_path
        } else {
            PathBuf::from(path)
        }
    } else {
        PathBuf::from(path)
    }
}

fn load_config() -> Result<Config> {
    // Try to load from config file, fallback to default
    let config_path = "git-monitor.toml";
    
    if std::path::Path::new(config_path).exists() {
        let content = std::fs::read_to_string(config_path)?;
        Ok(toml::from_str(&content)?)
    } else {
        // Default config
        Ok(Config {
            repositories: vec![
                RepoConfig {
                    name: "Current Directory".to_string(),
                    path: ".".to_string(),
                    remote: Some("origin".to_string()),
                }
            ],
            refresh_interval: 5,
            max_commits: 5,
        })
    }
}

fn get_repo_status(path: &PathBuf, remote: &str) -> Result<(usize, usize)> {
    let repo = Repository::open(path)?;
    
    // Try to fetch from remote (ignore errors for offline/network issues)
    if let Ok(mut remote_ref) = repo.find_remote(remote) {
        let _ = remote_ref.fetch(&[] as &[&str], None, None);
    }
    
    // Get local and remote branches
    let head = repo.head()?;
    let local_branch = head.shorthand().unwrap_or("main");
    let remote_branch = format!("{}/{}", remote, local_branch);
    
    let local_oid = head.target().unwrap();
    
    // Try to find remote branch, if it doesn't exist, assume 0 ahead/behind
    if let Ok(remote_ref) = repo.find_reference(&format!("refs/remotes/{}", remote_branch)) {
        if let Some(remote_oid) = remote_ref.target() {
            // Calculate ahead/behind
            let (ahead, behind) = repo.graph_ahead_behind(local_oid, remote_oid)?;
            return Ok((ahead, behind));
        }
    }
    
    // If no remote branch found, just return 0/0
    Ok((0, 0))
}

fn get_recent_commits(path: &PathBuf, count: usize) -> Vec<CommitInfo> {
    let mut commits = Vec::new();
    
    if let Ok(repo) = Repository::open(path) {
        if let Ok(mut revwalk) = repo.revwalk() {
            revwalk.push_head().ok();
            
            for (i, oid) in revwalk.enumerate() {
                if i >= count { break; }
                
                if let Ok(oid) = oid {
                    if let Ok(commit) = repo.find_commit(oid) {
                        commits.push(CommitInfo {
                            hash: format!("{:.8}", oid),
                            author: commit.author().name().unwrap_or("Unknown").to_string(),
                            message: commit.message().unwrap_or("No message").lines().next().unwrap_or("").to_string(),
                            timestamp: DateTime::from_timestamp(commit.time().seconds(), 0)
                                .unwrap_or_else(|| Utc::now()),
                        });
                    }
                }
            }
        }
    }
    
    commits
}

async fn monitor_repositories(
    repos: Arc<Mutex<Vec<RepoStatus>>>,
    console_messages: Arc<Mutex<Vec<ConsoleMessage>>>,
    refresh_interval: Duration,
) {
    let mut interval = time::interval(refresh_interval);
    
    loop {
        interval.tick().await;
        
        let mut repos_guard = repos.lock().unwrap();
        for repo in repos_guard.iter_mut() {
            let remote = "origin"; // Could be configurable
            
            // Always update the last_update time to show the monitor is running
            repo.last_update = Instant::now();
            
            match get_repo_status(&repo.path, remote) {
                Ok((ahead, behind)) => {
                    let prev_ahead = repo.ahead;
                    let prev_behind = repo.behind;
                    
                    repo.ahead = ahead;
                    repo.behind = behind;
                    
                    // Flash red if new commits behind
                    if behind > prev_behind {
                        repo.flash_until = Some(Instant::now() + Duration::from_secs(30));
                        repo.flash_color = Some(Color::Red);
                    }
                    
                    // Flash green if pulled (behind went to 0)
                    if prev_behind > 0 && behind == 0 {
                        repo.flash_until = Some(Instant::now() + Duration::from_secs(5));
                        repo.flash_color = Some(Color::Green);
                    }
                    
                    // Add console message for new commits
                    if ahead > prev_ahead {
                        let recent = get_recent_commits(&repo.path, (ahead - prev_ahead).min(5));
                        let mut console_guard = console_messages.lock().unwrap();
                        for commit in recent {
                            console_guard.push(ConsoleMessage {
                                timestamp: Utc::now(),
                                repo: repo.name.clone(),
                                author: commit.author,
                                message: commit.message,
                            });
                        }
                        // Keep only last 50 messages
                        let len = console_guard.len();
                        if len > 50 {
                            console_guard.drain(0..len - 50);
                        }
                    }
                }
                Err(err) => {
                    // If git operation fails, add a detailed console message
                    let mut console_guard = console_messages.lock().unwrap();
                    console_guard.push(ConsoleMessage {
                        timestamp: Utc::now(),
                        repo: repo.name.clone(),
                        author: "System".to_string(),
                        message: format!("Git error: {} (path: {})", err, repo.path.display()),
                    });
                }
            }
        }
        drop(repos_guard); // Release the lock before sleeping
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(6), Constraint::Length(3)].as_ref())
        .split(f.size());

    // Repository table
    let repos = app.repos.lock().unwrap();
    let now = Instant::now();
    
    let mut rows = Vec::new();
    for repo in repos.iter() {
        let style = if let Some(flash_until) = repo.flash_until {
            if now < flash_until {
                Style::default().fg(repo.flash_color.unwrap_or(Color::White))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            }
        } else {
            Style::default()
        };
        
        rows.push(Row::new(vec![
            Cell::from(repo.name.clone()),
            Cell::from(repo.ahead.to_string()),
            Cell::from(repo.behind.to_string()),
        ]).style(style));
        
        // Add expanded commits if selected
        if repo.expanded {
            for commit in &repo.recent_commits {
                rows.push(Row::new(vec![
                    Cell::from(format!("  └─ {} - {}", commit.hash, commit.message)),
                    Cell::from(commit.author.clone()),
                    Cell::from(""),
                ]).style(Style::default().fg(Color::Gray)));
            }
        }
    }
    
    let widths = [
        Constraint::Percentage(50),
        Constraint::Percentage(25),
        Constraint::Percentage(25),
    ];
    
    let table = Table::new(rows, widths)
        .block(Block::default().title("Git Repositories").borders(Borders::ALL))
        .header(Row::new(vec!["Repository", "Ahead", "Behind"])
            .style(Style::default().add_modifier(Modifier::BOLD)))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    
    f.render_stateful_widget(table, chunks[0], &mut app.table_state);
    
    // Console
    let console_messages = app.console_messages.lock().unwrap();
    let console_text = console_messages
        .iter()
        .rev()
        .take(4)
        .map(|msg| format!("[{}] {}: {} - {}", 
            msg.timestamp.format("%H:%M:%S"),
            msg.repo,
            msg.author,
            msg.message
        ))
        .collect::<Vec<_>>()
        .join("\n");
    
    let console = Paragraph::new(console_text)
        .block(Block::default().title("Console").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    
    f.render_widget(console, chunks[1]);
    
    // Help footer
    let help_text = "↑/↓: Navigate  Enter: Expand/Collapse  q: Quit";
    let help = Paragraph::new(help_text)
        .block(Block::default().title("Controls").borders(Borders::ALL))
        .style(Style::default().fg(Color::Gray));
    
    f.render_widget(help, chunks[2]);
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App, refresh_interval: Duration) -> Result<()> {
    // Start monitoring task
    let repos_clone = app.repos.clone();
    let console_clone = app.console_messages.clone();
    tokio::spawn(monitor_repositories(repos_clone, console_clone, refresh_interval));
    
    // UI loop
    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(250);
    
    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
            
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key.code);
            }
        }
        
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
        
        if app.should_quit {
            break;
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = load_config()?;
    let refresh_interval = Duration::from_secs(config.refresh_interval);
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Create app and run
    let app = App::new(config);
    
    // Add startup validation message
    {
        let repos = app.repos.lock().unwrap();
        let console_messages = app.console_messages.clone();
        let mut console_guard = console_messages.lock().unwrap();
        
        console_guard.push(ConsoleMessage {
            timestamp: Utc::now(),
            repo: "System".to_string(),
            author: "Git Monitor".to_string(),
            message: format!("Started monitoring {} repositories", repos.len()),
        });
        
        // Validate each repo path
        for repo in repos.iter() {
            if !repo.path.exists() {
                console_guard.push(ConsoleMessage {
                    timestamp: Utc::now(),
                    repo: repo.name.clone(),
                    author: "System".to_string(),
                    message: format!("Warning: Path does not exist: {}", repo.path.display()),
                });
            } else if !repo.path.join(".git").exists() {
                console_guard.push(ConsoleMessage {
                    timestamp: Utc::now(),
                    repo: repo.name.clone(),
                    author: "System".to_string(),
                    message: format!("Warning: Not a git repository: {}", repo.path.display()),
                });
            }
        }
    }
    
    let res = run_app(&mut terminal, app, refresh_interval).await;
    
    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    
    if let Err(err) = res {
        println!("{:?}", err);
    }
    
    Ok(())
}
