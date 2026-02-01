import os
import subprocess
import threading
import signal
import tkinter as tk
from tkinter import scrolledtext
from datetime import datetime
import ctypes
from ctypes import wintypes

# Windows API for window management
user32 = ctypes.windll.user32
SW_MINIMIZE = 6
SW_RESTORE = 9
SW_SHOWMINIMIZED = 2

# --- 1. CONFIGURATION ---
# The K_OS project root - hardcoded so script can run from ANYWHERE
PROJECT_ROOT = r"C:\Users\Admin\Desktop\K_OSNew"
SCRIPT_FOLDER = os.path.dirname(os.path.abspath(__file__))
DEST_ROOT = r"C:\Users\Admin\Desktop\autobackup"
WINRAR_EXE = r"C:\Program Files\WinRAR\WinRAR.exe"

# --- 2. EXCLUSIONS (Updated for Rust/Tauri) ---
# Directories to skip entirely
IGNORE_DIRS = {
    '.git', 'node_modules', 'backups', 'dist', 'build', 
    '.vs', '.idea', '__pycache__', 'coverage', '.next',
    'target',  # Rust build output (huge!)
    'gen',     # Tauri generated files
}

# File extensions to skip
IGNORE_EXTS = {
    '.zip', '.rar', '.7z',           # Archives
    '.bat',                           # Scripts (backup script itself)
    '.log', '.tmp',                   # Temp files
    '.lock',                          # Lock files (Cargo.lock, package-lock.json)
    '.kipp',                          # Large project files
    '.exe', '.dll', '.pdb',           # Compiled binaries
    '.ico',                           # Icons
}

class TitanBackupApp:
    def __init__(self, root):
        self.root = root
        self.root.title("Titan Backup V2 (Manual)")
        self.root.geometry("460x320")
        self.root.configure(bg="#0f0f0f")
        self.root.protocol("WM_DELETE_WINDOW", self.hide_main_window)
        
        self.bg_color = "#0f0f0f"
        self.panel_color = "#1a1a1a"
        self.accent_color = "#00ff9d"
        self.tauri_process = None  # Track running tauri dev server

        # --- UI LAYOUT ---
        self.header_frame = tk.Frame(root, bg=self.bg_color)
        self.header_frame.pack(fill="x", pady=15)
        self.status_label = tk.Label(self.header_frame, text="● READY", font=("Segoe UI", 11, "bold"), fg=self.accent_color, bg=self.bg_color)
        self.status_label.pack()

        self.info_frame = tk.Frame(root, bg=self.panel_color, padx=15, pady=10)
        self.info_frame.pack(fill="x", padx=20, pady=5)
        
        folder_name = os.path.basename(PROJECT_ROOT)
        self.project_label = tk.Label(self.info_frame, text=f"PROJECT: {folder_name.upper()}", font=("Consolas", 10, "bold"), fg="white", bg=self.panel_color)
        self.project_label.pack(anchor="w")
        
        self.last_run_label = tk.Label(self.info_frame, text="LAST SNAPSHOT: --:--", font=("Consolas", 9), fg="#888", bg=self.panel_color)
        self.last_run_label.pack(anchor="w")

        self.log_box = scrolledtext.ScrolledText(root, height=8, bg="#111", fg="#ddd", font=("Consolas", 8), borderwidth=0, highlightthickness=1, highlightbackground="#333")
        self.log_box.pack(padx=20, pady=15, fill="both", expand=True)

        # Button frame
        self.button_frame = tk.Frame(root, bg=self.bg_color)
        self.button_frame.pack(pady=10)

        self.force_btn = tk.Button(self.button_frame, text="BACKUP NOW", command=self.force_backup, bg="#222", fg="white", activebackground="#333", activeforeground="white", relief="flat", font=("Segoe UI", 9, "bold"))
        self.force_btn.pack(side="left", padx=5, ipadx=20, ipady=5)

        self.hide_btn = tk.Button(self.button_frame, text="HIDE WINDOW", command=self.hide_main_window, bg="#1a4d2e", fg="white", activebackground="#2d6a4f", activeforeground="white", relief="flat", font=("Segoe UI", 9, "bold"))
        self.hide_btn.pack(side="left", padx=5, ipadx=20, ipady=5)
        
        self.log("yer (Manual Mode)")
        self.log("No file watching - click BACKUP NOW when ready.")
        
        # Create floating overlay widget
        self.create_floating_widget()

    def create_floating_widget(self):
        """Create a clean, minimal floating backup widget"""
        self.widget = tk.Toplevel(self.root)
        self.widget.overrideredirect(True)
        self.widget.attributes("-topmost", True)
        self.widget.attributes("-alpha", 0.95)
        self.widget.configure(bg="#111111")
        
        # Position at top-center (larger default size for TODO tab)
        screen_w = self.widget.winfo_screenwidth()
        self.widget_width = 400
        self.widget_height = 320
        x_pos = (screen_w - self.widget_width) // 2
        self.widget.geometry(f"{self.widget_width}x{self.widget_height}+{x_pos}+15")
        
        # Track minimum size
        self.widget_min_width = 320
        self.widget_min_height = 200
        
        # Main container with subtle border
        main_frame = tk.Frame(self.widget, bg="#111111", highlightbackground="#333", highlightthickness=1)
        main_frame.pack(fill="both", expand=True)
        
        # Resize grip in bottom-right corner
        self.resize_grip = tk.Label(self.widget, text="⋱", font=("Arial", 10), fg="#444", bg="#111", cursor="size_nw_se")
        self.resize_grip.place(relx=1.0, rely=1.0, anchor="se")
        self.resize_grip.bind("<Button-1>", self.start_resize)
        self.resize_grip.bind("<B1-Motion>", self.on_resize)
        self.resize_grip.bind("<Enter>", lambda e: self.resize_grip.config(fg="#888"))
        self.resize_grip.bind("<Leave>", lambda e: self.resize_grip.config(fg="#444"))
        
        # --- HEADER ---
        header_frame = tk.Frame(main_frame, bg="#1a1a1a", pady=6)
        header_frame.pack(fill="x")
        
        # Backup Button (Green accent)
        self.widget_btn = tk.Label(header_frame, text="⚡ BACKUP", font=("Segoe UI", 9, "bold"), fg="#4caf50", bg="#1a1a1a", cursor="hand2", padx=10)
        self.widget_btn.pack(side="left")
        self.widget_btn.bind("<Button-1>", lambda e: self.force_backup())
        
        # Name Input (Minimal)
        self.backup_name_var = tk.StringVar()
        self.name_entry = tk.Entry(header_frame, textvariable=self.backup_name_var, bg="#111", fg="#ddd", insertbackground="#ddd", font=("Consolas", 9), relief="flat", width=14)
        self.name_entry.pack(side="left", padx=5, ipady=2)
        self.name_entry.insert(0, "Name...")
        self.name_entry.config(fg="#555")
        
        def on_entry_click(e):
            if self.name_entry.get() == "Name...":
                self.name_entry.delete(0, "end")
                self.name_entry.config(fg="#ddd")
        def on_focusout(e):
            if self.name_entry.get() == "":
                self.name_entry.insert(0, "Name...")
                self.name_entry.config(fg="#555")
        self.name_entry.bind('<FocusIn>', on_entry_click)
        self.name_entry.bind('<FocusOut>', on_focusout)
        self.name_entry.bind('<Return>', lambda e: self.force_backup())
        
        # Right Icons
        icons_frame = tk.Frame(header_frame, bg="#1a1a1a")
        icons_frame.pack(side="right", padx=5)
        
        restore_btn = tk.Label(icons_frame, text="↺", font=("Arial", 12), fg="#777", bg="#1a1a1a", cursor="hand2", padx=5)
        restore_btn.pack(side="left")
        restore_btn.bind("<Button-1>", lambda e: self.open_restore_menu())
        restore_btn.bind("<Enter>", lambda e: restore_btn.config(fg="#fff"))
        restore_btn.bind("<Leave>", lambda e: restore_btn.config(fg="#777"))
        
        settings_btn = tk.Label(icons_frame, text="⚙", font=("Arial", 11), fg="#777", bg="#1a1a1a", cursor="hand2", padx=5)
        settings_btn.pack(side="left")
        settings_btn.bind("<Button-1>", lambda e: self.open_settings())
        settings_btn.bind("<Enter>", lambda e: settings_btn.config(fg="#fff"))
        settings_btn.bind("<Leave>", lambda e: settings_btn.config(fg="#777"))
        
        # --- TAB BAR ---
        tab_frame = tk.Frame(main_frame, bg="#111")
        tab_frame.pack(fill="x", pady=(2, 0))
        
        self.tabs = {}
        self.current_tab = "CORE"
        
        def switch_tab(tab_name):
            self.current_tab = tab_name
            # Reset
            for name, lbl in self.tabs.items():
                lbl.config(fg="#666", bg="#111")
            # Active (Subtle distinct colors)
            color = "#fff"
            if tab_name == "CORE": color = "#64b5f6" # Soft Blue
            elif tab_name == "SCRIPTS": color = "#80cbc4" # Teal
            elif tab_name == "DEV": color = "#ffb74d" # Soft Orange
            elif tab_name == "UTIL": color = "#e1bee7" # Soft Purple
            elif tab_name == "CLIPBOARD": color = "#ffd54f" # Yellow
            elif tab_name == "GIT": color = "#f14e32" # Git Orange
            elif tab_name == "SCREENS": color = "#4fc3f7" # Cyan
            elif tab_name == "TODO": color = "#ff7043" # Deep Orange
            
            self.tabs[tab_name].config(fg=color, bg="#1a1a1a")
            
            for frame in [self.core_content, self.scripts_content, self.dev_content, self.util_content, self.clipboard_content, self.git_content, self.screens_content, self.todo_content]:
                frame.pack_forget()
                
            if tab_name == "CORE": self.core_content.pack(fill="both", expand=True, padx=8, pady=8)
            elif tab_name == "SCRIPTS": self.scripts_content.pack(fill="both", expand=True, padx=8, pady=8)
            elif tab_name == "DEV": self.dev_content.pack(fill="both", expand=True, padx=8, pady=8)
            elif tab_name == "UTIL": self.util_content.pack(fill="both", expand=True, padx=8, pady=8)
            elif tab_name == "CLIPBOARD": 
                self.refresh_clipboard_snippets()
                self.clipboard_content.pack(fill="both", expand=True, padx=8, pady=8)
            elif tab_name == "GIT": self.git_content.pack(fill="both", expand=True, padx=8, pady=8)
            elif tab_name == "SCREENS": 
                self.refresh_screenshots()
                self.screens_content.pack(fill="both", expand=True, padx=8, pady=8)
            elif tab_name == "TODO":
                self.refresh_todo_list()
                self.todo_content.pack(fill="both", expand=True, padx=8, pady=8)
        
        # Tab name mapping (for shorter display names)
        tab_display = {"CORE": "CORE", "SCRIPTS": "SCR", "DEV": "DEV", "UTIL": "UTIL", "CLIPBOARD": "CLIP", "GIT": "GIT", "SCREENS": "📷", "TODO": "✓"}
        for t in ["CORE", "SCRIPTS", "DEV", "UTIL", "CLIPBOARD", "GIT", "SCREENS", "TODO"]:
            display = tab_display.get(t, t)
            lbl = tk.Label(tab_frame, text=display, font=("Segoe UI", 7), fg="#666", bg="#111", cursor="hand2", width=6, pady=4)
            lbl.pack(side="left", fill="x", expand=True)
            lbl.bind("<Button-1>", lambda e, n=t: switch_tab(n))
            self.tabs[t] = lbl
            
        # Separator
        tk.Frame(main_frame, bg="#222", height=1).pack(fill="x")

        # --- CONTENT AREAS ---
        self.content_container = tk.Frame(main_frame, bg="#111")
        self.content_container.pack(fill="both", expand=True)

        # 1. CORE TAB
        self.core_content = tk.Frame(self.content_container, bg="#111")
        
        row1 = tk.Frame(self.core_content, bg="#111"); row1.pack(fill="x", pady=2)
        self.create_btn(row1, "LAUNCH K_OS", "#64b5f6", self.launch_tauri)
        self.create_btn(row1, "BEVY SOLO", "#a5d6a7", self.launch_bevy_standalone)
        self.create_btn(row1, "RESTART", "#ba68c8", self.restart_tauri)
        
        row2 = tk.Frame(self.core_content, bg="#111"); row2.pack(fill="x", pady=2)
        self.create_btn(row2, "NUKE RUST", "#e57373", self.nuke_rust_processes)

        
        # Folder Shortcuts
        row3 = tk.Frame(self.core_content, bg="#111"); row3.pack(fill="x", pady=2)
        self.create_btn(row3, "K_OS FOLDER", "#81c784", lambda: os.startfile(PROJECT_ROOT))
        self.create_btn(row3, "SCRIPTS", "#81c784", lambda: os.startfile(os.path.join(PROJECT_ROOT, "src-python", "windows_related_scripts")))
        self.create_btn(row3, "CLOSE ALL", "#e57373", self.close_all_explorers)
        
        # Window Management
        row4 = tk.Frame(self.core_content, bg="#111"); row4.pack(fill="x", pady=2)
        self.create_btn(row4, "TILE WINDOWS", "#4fc3f7", self.organize_windows)
        self.create_btn(row4, "MINIMIZE ALL", "#b0bec5", self.minimize_all)
        
        # 2. SCRIPTS TAB
        self.scripts_content = tk.Frame(self.content_container, bg="#111")
        
        s_row1 = tk.Frame(self.scripts_content, bg="#111"); s_row1.pack(fill="x", pady=2)
        self.create_btn(s_row1, "DL TEXTURE", "#80cbc4", lambda: os.startfile(r"C:\Users\Admin\Desktop\dl_texture.py"))
        self.create_btn(s_row1, "DL RIG MODEL", "#80cbc4", lambda: os.startfile(r"C:\Users\Admin\Desktop\dl_rigmodel.py"))
        self.create_btn(s_row1, "DL MODEL", "#80cbc4", lambda: os.startfile(r"C:\Users\Admin\Desktop\dl_model.py"))
        
        s_row2 = tk.Frame(self.scripts_content, bg="#111"); s_row2.pack(fill="x", pady=2)
        self.create_btn(s_row2, "DL HDRI", "#a5d6a7", lambda: os.startfile(r"C:\Users\Admin\Desktop\dl_hdri.py"))
        self.create_btn(s_row2, "ORGANIZE", "#a5d6a7", lambda: os.startfile(r"C:\Users\Admin\Desktop\organize_desktop.py"))
        self.create_btn(s_row2, "CONVERT IMG", "#a5d6a7", lambda: os.startfile(r"C:\Users\Admin\Desktop\img_convert.py"))

        # 3. DEV TAB
        self.dev_content = tk.Frame(self.content_container, bg="#111")
        
        # Row 1: Quick Actions
        d_row1 = tk.Frame(self.dev_content, bg="#111"); d_row1.pack(fill="x", pady=2)
        self.create_btn(d_row1, "LAUNCH K_OS", "#64b5f6", self.launch_tauri)
        self.create_btn(d_row1, "VS CODE", "#4fc3f7", self.open_vscode)
        self.create_btn(d_row1, "EXPLORER", "#fff176", self.open_explorer)
        
        # Row 2: Cargo Core Commands
        d_row2 = tk.Frame(self.dev_content, bg="#111"); d_row2.pack(fill="x", pady=2)
        self.create_btn(d_row2, "CARGO CHECK", "#ffb74d", lambda: self.run_cmd("cargo check", "Checking types...", cwd=os.path.join(PROJECT_ROOT, "src-tauri")))
        self.create_btn(d_row2, "CARGO BUILD", "#ffb74d", lambda: self.run_cmd("cargo build", "Building debug...", cwd=os.path.join(PROJECT_ROOT, "src-tauri")))
        self.create_btn(d_row2, "BUILD RELEASE", "#81c784", lambda: self.run_cmd("cargo build --release", "Building release...", cwd=os.path.join(PROJECT_ROOT, "src-tauri")))
        
        # Row 3: Cargo Maintenance
        d_row3 = tk.Frame(self.dev_content, bg="#111"); d_row3.pack(fill="x", pady=2)
        self.create_btn(d_row3, "CARGO CLEAN", "#e57373", lambda: self.run_cmd("cargo clean", "Cleaning target...", cwd=os.path.join(PROJECT_ROOT, "src-tauri")))
        self.create_btn(d_row3, "CARGO UPDATE", "#ba68c8", lambda: self.run_cmd("cargo update", "Updating deps...", cwd=os.path.join(PROJECT_ROOT, "src-tauri")))
        self.create_btn(d_row3, "CARGO DOC", "#4fc3f7", lambda: self.run_cmd("cargo doc --open", "Building docs...", cwd=os.path.join(PROJECT_ROOT, "src-tauri")))
        
        # Row 4: NPM & Git
        d_row4 = tk.Frame(self.dev_content, bg="#111"); d_row4.pack(fill="x", pady=2)
        self.create_btn(d_row4, "NPM INSTALL", "#fff176", lambda: self.run_cmd("npm install", "Installing..."))
        self.create_btn(d_row4, "GIT STATUS", "#e57373", self.show_git_status)
        self.create_btn(d_row4, "PROD BUILD", "#81c784", lambda: self.run_cmd("npm run tauri:build", "Building prod..."))

        # 4. UTIL TAB
        self.util_content = tk.Frame(self.content_container, bg="#111")
        
        u_row1 = tk.Frame(self.util_content, bg="#111"); u_row1.pack(fill="x", pady=2)
        self.create_btn(u_row1, "SCAN DEAD CODE", "#ce93d8", self.scan_dead_code)
        self.create_btn(u_row1, "KILL PORTS", "#b39ddb", self.kill_ports)
        
        u_row2 = tk.Frame(self.util_content, bg="#111"); u_row2.pack(fill="x", pady=2)
        self.create_btn(u_row2, "CLEAR CACHE", "#e57373", self.clear_all_cache)
        self.create_btn(u_row2, "KILL SCRIPT", "#e57373", self.kill_all_buttons)
        
        # 5. CLIPBOARD TAB (Snippet Manager)
        self.clipboard_content = tk.Frame(self.content_container, bg="#111")
        self.clipboard_folder = r"C:\Users\Admin\Desktop\universalcopy"
        
        # Ensure folder exists
        os.makedirs(self.clipboard_folder, exist_ok=True)
        
        # + Button to create new snippet
        add_row = tk.Frame(self.clipboard_content, bg="#111")
        add_row.pack(fill="x", pady=2)
        self.create_btn(add_row, "+ NEW SNIPPET", "#ffd54f", self.create_new_snippet)
        
        # Scrollable snippet list
        self.clipboard_scroll_frame = tk.Frame(self.clipboard_content, bg="#111")
        self.clipboard_scroll_frame.pack(fill="both", expand=True)
        
        # 6. GIT TAB (Version Control Made Easy!)
        self.git_content = tk.Frame(self.content_container, bg="#111")
        
        # Row 1: View commands
        g_row1 = tk.Frame(self.git_content, bg="#111"); g_row1.pack(fill="x", pady=2)
        self.create_btn(g_row1, "STATUS", "#f14e32", self.git_status_popup)
        self.create_btn(g_row1, "DIFF", "#ffb74d", self.git_diff_popup)
        self.create_btn(g_row1, "LOG", "#64b5f6", self.git_log_popup)
        
        # Row 2: Stage & Commit
        g_row2 = tk.Frame(self.git_content, bg="#111"); g_row2.pack(fill="x", pady=2)
        self.create_btn(g_row2, "ADD ALL", "#81c784", self.git_add_all)
        self.create_btn(g_row2, "COMMIT", "#4caf50", self.git_commit_dialog)
        
        # Row 3: Dangerous actions
        g_row3 = tk.Frame(self.git_content, bg="#111"); g_row3.pack(fill="x", pady=2)
        self.create_btn(g_row3, "⚠ UNDO CHANGES", "#e57373", self.git_undo_changes)
        
        # Row 4: Help/Key button
        g_row4 = tk.Frame(self.git_content, bg="#111"); g_row4.pack(fill="x", pady=2)
        self.create_btn(g_row4, "GIT CHEATSHEET", "#ce93d8", self.git_show_key)
        
        # 7. SCREENS TAB (Screenshot Gallery)
        self.screens_content = tk.Frame(self.content_container, bg="#111")
        self.screenshots_folder = r"C:\Users\Admin\Desktop\SCREENSHOTS"
        
        # Ensure folder exists
        os.makedirs(self.screenshots_folder, exist_ok=True)
        
        # Action buttons row
        screens_action_row = tk.Frame(self.screens_content, bg="#111")
        screens_action_row.pack(fill="x", pady=2)
        self.create_btn(screens_action_row, "📂 OPEN FOLDER", "#4fc3f7", lambda: os.startfile(self.screenshots_folder))
        self.create_btn(screens_action_row, "🔄 REFRESH", "#81c784", self.refresh_screenshots)
        
        # Scrollable screenshot list
        self.screens_scroll_frame = tk.Frame(self.screens_content, bg="#111")
        self.screens_scroll_frame.pack(fill="both", expand=True)
        
        # 8. TODO TAB (Task Manager with Sub-tabs)
        self.todo_content = tk.Frame(self.content_container, bg="#111")
        self.todo_file = os.path.join(SCRIPT_FOLDER, "todos.json")
        self.current_todo_category = "TODO"  # Default category
        
        # Load or initialize todos
        self.todos = self.load_todos()
        
        # Sub-tab bar for TODO categories
        todo_subtab_frame = tk.Frame(self.todo_content, bg="#1a1a1a")
        todo_subtab_frame.pack(fill="x", pady=(0, 4))
        
        self.todo_subtabs = {}
        self.todo_categories = ["TODO", "BUGS", "FINISH"]
        todo_colors = {"TODO": "#ff7043", "BUGS": "#e57373", "FINISH": "#81c784"}
        
        def switch_todo_subtab(category):
            self.current_todo_category = category
            for cat, btn in self.todo_subtabs.items():
                if cat == category:
                    btn.config(fg=todo_colors[cat], bg="#252525")
                else:
                    btn.config(fg="#666", bg="#1a1a1a")
            self.refresh_todo_list()
        
        for cat in self.todo_categories:
            btn = tk.Label(
                todo_subtab_frame, text=cat, font=("Segoe UI", 8, "bold"),
                fg="#666", bg="#1a1a1a", cursor="hand2", padx=12, pady=4
            )
            btn.pack(side="left")
            btn.bind("<Button-1>", lambda e, c=cat: switch_todo_subtab(c))
            btn.bind("<Enter>", lambda e, b=btn, c=cat: b.config(bg="#252525") if c != self.current_todo_category else None)
            btn.bind("<Leave>", lambda e, b=btn, c=cat: b.config(bg="#1a1a1a") if c != self.current_todo_category else None)
            self.todo_subtabs[cat] = btn
        
        # + Add button on the right
        add_todo_btn = tk.Label(
            todo_subtab_frame, text="+", font=("Arial", 12, "bold"),
            fg="#81c784", bg="#1a1a1a", cursor="hand2", padx=10
        )
        add_todo_btn.pack(side="right")
        add_todo_btn.bind("<Button-1>", lambda e: self.add_todo_item())
        add_todo_btn.bind("<Enter>", lambda e: add_todo_btn.config(fg="#a5d6a7", bg="#252525"))
        add_todo_btn.bind("<Leave>", lambda e: add_todo_btn.config(fg="#81c784", bg="#1a1a1a"))
        
        # Scrollable todo list (MUST be created before switch_todo_subtab)
        self.todo_scroll_frame = tk.Frame(self.todo_content, bg="#111")
        self.todo_scroll_frame.pack(fill="both", expand=True)
        
        # Init first subtab
        switch_todo_subtab("TODO")
        
        # --- FOOTER (Status & Timestamp) ---
        footer_frame = tk.Frame(main_frame, bg="#111", pady=4)
        footer_frame.pack(side="bottom", fill="x", padx=10, pady=(2, 6))
        
        self.tauri_status_label = tk.Label(footer_frame, text="K_OS: stopped", font=("Consolas", 8), fg="#666", bg="#111")
        self.tauri_status_label.pack(side="left")
        
        self.widget_time_label = tk.Label(footer_frame, text="Last backup: --:--", font=("Consolas", 8), fg="#444", bg="#111")
        self.widget_time_label.pack(side="right")
        
        # Init Tab
        switch_tab("CORE")
        
        # Dragging
        for w in [self.widget, main_frame, header_frame, tab_frame]:
            w.bind("<Button-1>", self.start_drag)
            w.bind("<B1-Motion>", self.on_drag)

    def create_btn(self, parent, text, color, cmd):
        # Minimal button style: text only, hover effect
        btn = tk.Label(parent, text=text, font=("Segoe UI", 8), fg=color, bg="#1a1a1a", cursor="hand2", pady=5)
        btn.pack(side="left", fill="x", expand=True, padx=2)
        btn.bind("<Button-1>", lambda e: cmd())
        btn.bind("<Enter>", lambda e: btn.config(bg="#252525"))
        btn.bind("<Leave>", lambda e: btn.config(bg="#1a1a1a"))

        
    def start_drag(self, event):
        """Start dragging the widget"""
        self.drag_x = event.x_root - self.widget.winfo_x()
        self.drag_y = event.y_root - self.widget.winfo_y()
        
    def on_drag(self, event):
        """Handle widget dragging"""
        x = event.x_root - self.drag_x
        y = event.y_root - self.drag_y
        self.widget.geometry(f"+{x}+{y}")
    
    def start_resize(self, event):
        """Start resizing the widget"""
        self.resize_start_x = event.x_root
        self.resize_start_y = event.y_root
        self.resize_start_w = self.widget.winfo_width()
        self.resize_start_h = self.widget.winfo_height()
    
    def on_resize(self, event):
        """Handle widget resizing"""
        delta_x = event.x_root - self.resize_start_x
        delta_y = event.y_root - self.resize_start_y
        
        new_w = max(self.widget_min_width, self.resize_start_w + delta_x)
        new_h = max(self.widget_min_height, self.resize_start_h + delta_y)
        
        self.widget.geometry(f"{new_w}x{new_h}")
    
    def show_main_window(self):
        """Show the main window"""
        self.root.deiconify()
        self.root.lift()
        self.root.focus_force()
    
    def hide_main_window(self):
        """Hide the main window"""
        self.root.withdraw()
    
    def update_widget_time(self, time_str):
        """Update the widget timestamp"""
        try:
            self.widget_time_label.config(text=f"Last backup: {time_str}")
        except:
            pass
    
    def launch_tauri(self):
        """Launch K_OS using the reliable batch file"""
        if self.tauri_process and self.tauri_process.poll() is None:
            self.log("K_OS already running! Use RESTART to reload.")
            return
        
        try:
            self.log("Launching via bat...")
            self.tauri_status_label.config(text="K_OS: starting...", fg="#00bfff")
            
            # Use os.startfile - most reliable way to run a bat
            bat_path = r"C:\Users\Admin\Desktop\run_tauri_dev.bat"
            os.startfile(bat_path)
            
            self.tauri_status_label.config(text="K_OS: running", fg="#00ff9d")
            self.log("✓ K_OS launched!")
            
        except Exception as e:
            self.log(f"Launch error: {e}")
            self.tauri_status_label.config(text="K_OS: error", fg="#ff6b6b")
    
    def launch_bevy_standalone(self):
        """Launch Bevy viewport in standalone mode (no Tauri/React sync).
        
        This runs the Bevy binary directly with --standalone flag,
        giving you a movable, resizable window with all sculpt features.
        No IPC sync with Tauri - pure Bevy experience!
        """
        self.log("Launching Bevy standalone...")
        self.tauri_status_label.config(text="Bevy: starting...", fg="#a5d6a7")
        
        def do_launch():
            try:
                # Run cargo with --standalone flag
                cmd = "cargo run --bin k-os-bevy -- --standalone"
                cwd = os.path.join(PROJECT_ROOT, "src-tauri")
                
                # Use Popen to start in background - WITH visible console for debugging!
                process = subprocess.Popen(
                    cmd,
                    cwd=cwd,
                    shell=True,
                    # No CREATE_NO_WINDOW - we WANT to see the terminal for debugging
                )
                
                self.root.after(0, lambda: self.tauri_status_label.config(text="Bevy: running", fg="#a5d6a7"))
                self.log("✓ Bevy standalone launched!")
                self.log("   Window is movable and resizable!")
                
            except Exception as e:
                self.log(f"Bevy launch error: {e}")
                self.root.after(0, lambda: self.tauri_status_label.config(text="Bevy: error", fg="#ff6b6b"))

        
        threading.Thread(target=do_launch, daemon=True).start()
    
    def stop_tauri(self):

        """Stop the running tauri dev process"""
        if not self.tauri_process or self.tauri_process.poll() is not None:
            self.log("K_OS not running.")
            return False
        
        try:
            self.log("Stopping K_OS...")
            self.tauri_status_label.config(text="K_OS: stopping...", fg="#ff9f00")
            
            # Kill entire process tree (npm spawns child processes)
            subprocess.run(
                f"taskkill /F /T /PID {self.tauri_process.pid}",
                shell=True,
                creationflags=subprocess.CREATE_NO_WINDOW
            )
            
            self.tauri_process = None
            self.tauri_status_label.config(text="K_OS: stopped", fg="#555")
            self.log("✓ K_OS stopped.")
            return True
            
        except Exception as e:
            self.log(f"Stop error: {e}")
            return False
    
    def restart_tauri(self):
        """Restart tauri dev (stop + launch)"""
        self.log("Restarting K_OS...")
        
        def do_restart():
            self.stop_tauri()
            import time
            time.sleep(1)  # Brief pause to let processes clean up
            self.launch_tauri()
        
        threading.Thread(target=do_restart, daemon=True).start()
    
    def nuke_rust_processes(self):
        """Kill ALL Rust-related processes and clear file locks.
        
        Fixes the dreaded 'file is being used by another process' error (os error 32)
        that happens during cargo builds.
        """
        self.log("NUKING all Rust processes...")
        
        def do_nuke():
            import time
            import shutil
            
            # List of Rust-related processes to kill
            processes_to_kill = [
                "cargo.exe",
                "rustc.exe", 
                "rust-analyzer.exe",
                "k-os-backend.exe",
                "k-os-bevy.exe",
                "node.exe",  # npm spawns node
            ]
            
            killed_count = 0
            
            for proc in processes_to_kill:
                try:
                    result = subprocess.run(
                        f"taskkill /F /IM {proc} /T",
                        shell=True,
                        capture_output=True,
                        creationflags=subprocess.CREATE_NO_WINDOW
                    )
                    if result.returncode == 0:
                        self.log(f"  ✓ Killed {proc}")
                        killed_count += 1
                except:
                    pass
            
            # Wait for processes to fully terminate
            time.sleep(1)
            
            # Clear stuck cache files (the infamous ash library issue)
            # UPDATED: Now checking root target folder due to workspace structure
            target_path = os.path.join(PROJECT_ROOT, "target", "debug", "deps")
            legacy_target_path = os.path.join(PROJECT_ROOT, "src-tauri", "target", "debug", "deps")
            
            paths_to_check = [target_path, legacy_target_path]
            
            for path in paths_to_check:
                if os.path.exists(path):
                    try:
                        # Remove problematic ash library files  
                        for f in os.listdir(path):
                            if f.startswith("libash"):
                                fpath = os.path.join(path, f)
                                try:
                                    os.remove(fpath)
                                    self.log(f"  ✓ Cleared: {f}")
                                except:
                                    pass
                    except Exception as e:
                        self.log(f"  Cache clear error ({os.path.basename(path)}): {e}")
            
            self.tauri_process = None
            self.tauri_status_label.config(text="K_OS: nuked", fg="#ff6b6b")
            self.log(f"NUKE complete! Killed {killed_count} processes.")
            self.log("   You can now run LAUNCH to start fresh.")
            
            # Reset status after a moment
            time.sleep(2)
            self.tauri_status_label.config(text="K_OS: stopped", fg="#555")
        
        threading.Thread(target=do_nuke, daemon=True).start()
    
    def scan_dead_code(self):
        """Scan for unused files, exports, and dependencies using knip"""
        self.log("Scanning for dead code...")
        
        def do_scan():
            import time
            
            # Create results window
            def create_results_window(output, errors):
                results = tk.Toplevel(self.root)
                results.title("Dead Code Scan Results")
                results.geometry("800x600")
                results.configure(bg="#0f0f0f")
                results.attributes("-topmost", True)
                
                # Header
                header = tk.Label(
                    results, 
                    text="DEAD CODE SCAN RESULTS", 
                    font=("Segoe UI", 14, "bold"), 
                    fg="#b388ff", 
                    bg="#0f0f0f"
                )
                header.pack(pady=15)
                
                # Results text area
                text_area = scrolledtext.ScrolledText(
                    results, 
                    bg="#111", 
                    fg="#ddd", 
                    font=("Consolas", 9),
                    borderwidth=0, 
                    highlightthickness=1, 
                    highlightbackground="#333"
                )
                text_area.pack(padx=20, pady=10, fill="both", expand=True)
                
                if errors:
                    text_area.insert(tk.END, "=== ERRORS ===\n", "error")
                    text_area.insert(tk.END, errors + "\n\n")
                
                if output:
                    text_area.insert(tk.END, output)
                else:
                    text_area.insert(tk.END, "✓ No dead code found! Your codebase is clean.\n")
                
                text_area.config(state="disabled")
                
                # Close button
                close_btn = tk.Button(
                    results,
                    text="CLOSE",
                    command=results.destroy,
                    bg="#222",
                    fg="white",
                    activebackground="#333",
                    activeforeground="white",
                    relief="flat",
                    font=("Segoe UI", 10, "bold"),
                    cursor="hand2"
                )
                close_btn.pack(pady=15, ipadx=40, ipady=8)
            
            try:
                # PROJECT_ROOT is already the K_OS project root
                self.log(f"  Scanning: {PROJECT_ROOT}")
                
                # Run knip (will auto-install if not present via npx)
                result = subprocess.run(
                    "npx knip --no-progress",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                output = result.stdout
                errors = result.stderr
                
                # Schedule window creation on main thread
                self.root.after(0, lambda: create_results_window(output, errors))
                
                # Summary in log
                if output.strip():
                    lines = output.strip().split('\n')
                    self.log(f"⚠ Found {len(lines)} potential issues")
                else:
                    self.log("✓ Scan complete - codebase looks clean!")
                    
            except Exception as e:
                self.log(f"Scan error: {e}")
                self.root.after(0, lambda: create_results_window("", str(e)))
        
        threading.Thread(target=do_scan, daemon=True).start()
    
    def kill_all_buttons(self):
        """Terminate the entire ilovebuttons script.
        
        Gracefully stops any running K_OS process and exits.
        """
        self.log("☠ KILLING ALL BUTTONS...")
        
        # Stop tauri if running
        if self.tauri_process and self.tauri_process.poll() is None:
            self.log("  Stopping K_OS first...")
            try:
                subprocess.run(
                    f"taskkill /F /T /PID {self.tauri_process.pid}",
                    shell=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
            except:
                pass
        
        self.log("Goodbye!")
        
        # Destroy everything and exit
        self.root.after(500, self.root.destroy)
    
    def run_cmd(self, cmd, log_msg, cwd=None):
        """Run a command in a background thread with output popup."""
        if cwd is None:
            cwd = PROJECT_ROOT
        
        self.log(log_msg)
        
        def do_run():
            try:
                result = subprocess.run(
                    cmd,
                    cwd=cwd,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                output = result.stdout + result.stderr
                
                # Show output in popup
                def show_output():
                    popup = tk.Toplevel(self.root)
                    popup.title(f"Command Output: {cmd}")
                    popup.geometry("700x400")
                    popup.configure(bg="#0f0f0f")
                    popup.attributes("-topmost", True)
                    
                    text = scrolledtext.ScrolledText(popup, bg="#111", fg="#ddd", font=("Consolas", 9))
                    text.pack(padx=10, pady=10, fill="both", expand=True)
                    text.insert(tk.END, output if output.strip() else "✓ Command completed successfully (no output)")
                    text.config(state="disabled")
                    
                    tk.Button(popup, text="CLOSE", command=popup.destroy, bg="#222", fg="white", relief="flat").pack(pady=10)
                
                self.root.after(0, show_output)
                
                if result.returncode == 0:
                    self.log(f"✓ {cmd} completed!")
                else:
                    self.log(f"⚠ {cmd} finished with errors")
                    
            except Exception as e:
                self.log(f"Error running {cmd}: {e}")
        
        threading.Thread(target=do_run, daemon=True).start()
    
    def open_vscode(self):
        """Open project in VS Code."""
        self.log("Opening VS Code...")
        try:
            subprocess.Popen(f'code "{PROJECT_ROOT}"', shell=True, creationflags=subprocess.CREATE_NO_WINDOW)
            self.log("✓ VS Code opened!")
        except Exception as e:
            self.log(f"Error opening VS Code: {e}")
    
    def open_explorer(self):
        """Open project folder in Windows Explorer."""
        self.log("Opening Explorer...")
        try:
            os.startfile(PROJECT_ROOT)
            self.log("✓ Explorer opened!")
        except Exception as e:
            self.log(f"Error opening Explorer: {e}")
    
    def show_git_status(self):
        """Show git status in a popup."""
        self.log("Getting git status...")
        
        def do_git():
            try:
                # Get status
                status = subprocess.run(
                    "git status",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                # Get recent commits
                log = subprocess.run(
                    "git log --oneline -10",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                def show_popup():
                    popup = tk.Toplevel(self.root)
                    popup.title("Git Status")
                    popup.geometry("700x500")
                    popup.configure(bg="#0f0f0f")
                    popup.attributes("-topmost", True)
                    
                    tk.Label(popup, text="GIT STATUS", font=("Segoe UI", 14, "bold"), fg="#f14e32", bg="#0f0f0f").pack(pady=10)
                    
                    text = scrolledtext.ScrolledText(popup, bg="#111", fg="#ddd", font=("Consolas", 9))
                    text.pack(padx=10, pady=5, fill="both", expand=True)
                    
                    text.insert(tk.END, "=== STATUS ===\n")
                    text.insert(tk.END, status.stdout + status.stderr + "\n\n")
                    text.insert(tk.END, "=== RECENT COMMITS ===\n")
                    text.insert(tk.END, log.stdout + log.stderr)
                    text.config(state="disabled")
                    
                    btn_frame = tk.Frame(popup, bg="#0f0f0f")
                    btn_frame.pack(pady=10)
                    
                    tk.Button(btn_frame, text="CLOSE", command=popup.destroy, bg="#222", fg="white", relief="flat").pack(side="left", padx=5)
                
                self.root.after(0, show_popup)
                self.log("✓ Git status loaded!")
                
            except Exception as e:
                self.log(f"Git error: {e}")
        
        threading.Thread(target=do_git, daemon=True).start()
    
    # ========== GIT TAB METHODS ==========
    
    def git_status_popup(self):
        """Show git status in a nice popup."""
        self.log("Getting git status...")
        
        def do_git():
            try:
                status = subprocess.run(
                    "git status",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                def show_popup():
                    popup = tk.Toplevel(self.root)
                    popup.title("Git Status")
                    popup.geometry("700x500")
                    popup.configure(bg="#0f0f0f")
                    popup.attributes("-topmost", True)
                    
                    tk.Label(popup, text="GIT STATUS", font=("Segoe UI", 14, "bold"), fg="#f14e32", bg="#0f0f0f").pack(pady=10)
                    
                    text = scrolledtext.ScrolledText(popup, bg="#111", fg="#ddd", font=("Consolas", 9))
                    text.pack(padx=10, pady=5, fill="both", expand=True)
                    text.insert(tk.END, status.stdout + status.stderr)
                    text.config(state="disabled")
                    
                    tk.Button(popup, text="CLOSE", command=popup.destroy, bg="#222", fg="white", relief="flat").pack(pady=10)
                
                self.root.after(0, show_popup)
                self.log("✓ Git status loaded!")
                
            except Exception as e:
                self.log(f"Git error: {e}")
        
        threading.Thread(target=do_git, daemon=True).start()
    
    def git_diff_popup(self):
        """Show what's changed in files (git diff)."""
        self.log("Getting git diff...")
        
        def do_diff():
            try:
                # Get unstaged changes
                diff = subprocess.run(
                    "git diff --stat",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                # Also get staged changes
                staged = subprocess.run(
                    "git diff --staged --stat",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                def show_popup():
                    popup = tk.Toplevel(self.root)
                    popup.title("Git Diff")
                    popup.geometry("750x500")
                    popup.configure(bg="#0f0f0f")
                    popup.attributes("-topmost", True)
                    
                    tk.Label(popup, text="📝 WHAT'S CHANGED?", font=("Segoe UI", 14, "bold"), fg="#ffb74d", bg="#0f0f0f").pack(pady=10)
                    
                    text = scrolledtext.ScrolledText(popup, bg="#111", fg="#ddd", font=("Consolas", 9))
                    text.pack(padx=10, pady=5, fill="both", expand=True)
                    
                    if staged.stdout.strip():
                        text.insert(tk.END, "=== STAGED (ready to commit) ===\n", "green")
                        text.insert(tk.END, staged.stdout + "\n\n")
                    
                    if diff.stdout.strip():
                        text.insert(tk.END, "=== UNSTAGED (not added yet) ===\n", "yellow")
                        text.insert(tk.END, diff.stdout)
                    
                    if not staged.stdout.strip() and not diff.stdout.strip():
                        text.insert(tk.END, "✓ No changes! Working directory is clean.\n")
                    
                    text.config(state="disabled")
                    
                    tk.Button(popup, text="CLOSE", command=popup.destroy, bg="#222", fg="white", relief="flat").pack(pady=10)
                
                self.root.after(0, show_popup)
                self.log("✓ Diff loaded!")
                
            except Exception as e:
                self.log(f"Diff error: {e}")
        
        threading.Thread(target=do_diff, daemon=True).start()
    
    def git_log_popup(self):
        """Show recent commits with full messages."""
        self.log("Getting git log...")
        
        def do_log():
            try:
                log = subprocess.run(
                    'git log --oneline -20 --format="%h  %s  (%ar)"',
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                def show_popup():
                    popup = tk.Toplevel(self.root)
                    popup.title("Git Log")
                    popup.geometry("800x500")
                    popup.configure(bg="#0f0f0f")
                    popup.attributes("-topmost", True)
                    
                    tk.Label(popup, text="RECENT COMMITS (Last 20)", font=("Segoe UI", 14, "bold"), fg="#64b5f6", bg="#0f0f0f").pack(pady=10)
                    
                    text = scrolledtext.ScrolledText(popup, bg="#111", fg="#ddd", font=("Consolas", 10))
                    text.pack(padx=10, pady=5, fill="both", expand=True)
                    text.insert(tk.END, log.stdout + log.stderr)
                    text.config(state="disabled")
                    
                    tk.Button(popup, text="CLOSE", command=popup.destroy, bg="#222", fg="white", relief="flat").pack(pady=10)
                
                self.root.after(0, show_popup)
                self.log("✓ Git log loaded!")
                
            except Exception as e:
                self.log(f"Log error: {e}")
        
        threading.Thread(target=do_log, daemon=True).start()
    
    def git_add_all(self):
        """Stage all changes (git add .)"""
        self.log("Staging all changes...")
        
        def do_add():
            try:
                result = subprocess.run(
                    "git add .",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                if result.returncode == 0:
                    self.log("✓ All changes staged! Ready to commit.")
                else:
                    self.log(f"⚠ Add error: {result.stderr}")
                    
            except Exception as e:
                self.log(f"Add error: {e}")
        
        threading.Thread(target=do_add, daemon=True).start()
    
    def git_commit_dialog(self):
        """Open dialog for commit message and commit."""
        from tkinter import simpledialog
        
        # Check if there are staged changes first
        check = subprocess.run(
            "git diff --staged --quiet",
            cwd=PROJECT_ROOT,
            shell=True,
            creationflags=subprocess.CREATE_NO_WINDOW
        )
        
        if check.returncode == 0:
            # No staged changes
            self.log("⚠ Nothing to commit! Use ADD ALL first.")
            return
        
        message = simpledialog.askstring(
            "Git Commit", 
            "Enter commit message:\n\n(Tip: Keep it short & descriptive)",
            parent=self.widget
        )
        
        if not message or not message.strip():
            self.log("Commit cancelled.")
            return
        
        self.log(f"Committing: {message[:50]}...")
        
        def do_commit():
            try:
                # Escape quotes in message
                safe_msg = message.replace('"', '\\"')
                result = subprocess.run(
                    f'git commit -m "{safe_msg}"',
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                if result.returncode == 0:
                    self.log("✓ Commit successful!")
                    # Show brief summary
                    lines = result.stdout.strip().split('\n')
                    if lines:
                        self.log(f"  {lines[0]}")
                else:
                    self.log(f"⚠ Commit error: {result.stderr}")
                    
            except Exception as e:
                self.log(f"Commit error: {e}")
        
        threading.Thread(target=do_commit, daemon=True).start()
    
    def git_undo_changes(self):
        """Discard all unstaged changes (DANGEROUS!)"""
        import tkinter.messagebox as mb
        
        # Confirm this dangerous action
        if not mb.askyesno(
            "⚠️ UNDO ALL CHANGES?",
            "This will DISCARD all your uncommitted changes!\n\n"
            "• Modified files → reverted\n"
            "• New files → kept (untracked)\n"
            "• Staged changes → unstaged\n\n"
            "This CANNOT be undone. Are you sure?",
            parent=self.widget,
            icon="warning"
        ):
            self.log("Undo cancelled.")
            return
        
        self.log("🔄 Undoing all changes...")
        
        def do_undo():
            try:
                # First unstage everything
                subprocess.run(
                    "git reset HEAD",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                # Then discard changes
                result = subprocess.run(
                    "git checkout -- .",
                    cwd=PROJECT_ROOT,
                    shell=True,
                    capture_output=True,
                    text=True,
                    creationflags=subprocess.CREATE_NO_WINDOW
                )
                
                if result.returncode == 0:
                    self.log("✓ All changes undone! Back to last commit.")
                else:
                    self.log(f"⚠ Undo error: {result.stderr}")
                    
            except Exception as e:
                self.log(f"Undo error: {e}")
        
        threading.Thread(target=do_undo, daemon=True).start()
    
    def git_show_key(self):
        """Show a helpful Git cheatsheet popup."""
        popup = tk.Toplevel(self.root)
        popup.title("Git Cheatsheet")
        popup.geometry("550x480")
        popup.configure(bg="#0f0f0f")
        popup.attributes("-topmost", True)
        
        # Header
        tk.Label(popup, text="GIT CHEATSHEET", font=("Segoe UI", 16, "bold"), fg="#ce93d8", bg="#0f0f0f").pack(pady=15)
        tk.Label(popup, text="Learn Git the easy way!", font=("Consolas", 9), fg="#888", bg="#0f0f0f").pack()
        
        # Content frame
        content = tk.Frame(popup, bg="#1a1a1a")
        content.pack(fill="both", expand=True, padx=20, pady=15)
        
        # Git concepts explained
        explanations = [
            ("📊 STATUS", "#f14e32", 
             "Shows what files have changed.\n• Modified files (you edited)\n• New files (untracked)\n• Staged files (ready to commit)"),
            
            ("📝 DIFF", "#ffb74d",
             "Shows WHAT changed in each file.\n• Green lines = added\n• Red lines = removed\n• Staged vs unstaged changes"),
            
            ("📜 LOG", "#64b5f6",
             "Shows your save history (commits).\n• Each commit = a snapshot in time\n• You can go back to any commit!\n• Shows who made changes & when"),
            
            ("➕ ADD ALL", "#81c784",
             "Stages ALL your changes.\n• Prepares files for committing\n• Like putting items in a box\n• Nothing is saved yet!"),
            
            ("💾 COMMIT", "#4caf50",
             "Saves your staged changes forever.\n• Creates a checkpoint\n• Add a message describing changes\n• This is your LOCAL save point"),
            
            ("⚠️ UNDO", "#e57373",
             "DISCARDS all uncommitted changes!\n• Goes back to last commit\n• CANNOT be undone!\n• Use with caution"),
        ]
        
        for i, (button, color, desc) in enumerate(explanations):
            frame = tk.Frame(content, bg="#1a1a1a")
            frame.pack(fill="x", pady=4)
            
            # Button name
            lbl = tk.Label(frame, text=button, font=("Segoe UI", 9, "bold"), fg=color, bg="#1a1a1a", width=12, anchor="w")
            lbl.pack(side="left", padx=(10, 5))
            
            # Description
            desc_lbl = tk.Label(frame, text=desc, font=("Consolas", 8), fg="#bbb", bg="#1a1a1a", justify="left", anchor="w")
            desc_lbl.pack(side="left", fill="x", expand=True, padx=5)
        
        # Footer with workflow tip
        footer = tk.Frame(popup, bg="#0f0f0f")
        footer.pack(fill="x", padx=20, pady=10)
        
        tk.Label(footer, text="💡 TYPICAL WORKFLOW:", font=("Segoe UI", 9, "bold"), fg="#ffd54f", bg="#0f0f0f").pack(anchor="w")
        tk.Label(footer, text="1. Make changes → 2. STATUS → 3. ADD ALL → 4. COMMIT", font=("Consolas", 9), fg="#888", bg="#0f0f0f").pack(anchor="w")
        
        tk.Button(popup, text="GOT IT!", command=popup.destroy, bg="#222", fg="white", relief="flat", font=("Segoe UI", 10, "bold")).pack(pady=15)
    
    def clear_all_cache(self):
        """Nuclear option: Clear all caches (cargo target + node_modules/.cache)."""
        self.log("🔥 CLEARING ALL CACHES...")
        
        def do_clear():
            import shutil
            import time
            
            cleared = 0
            
            # Clear cargo target
            target = os.path.join(PROJECT_ROOT, "src-tauri", "target")
            if os.path.exists(target):
                try:
                    self.log("  Clearing cargo target...")
                    shutil.rmtree(target)
                    cleared += 1
                    self.log("  ✓ Cargo target cleared!")
                except Exception as e:
                    self.log(f"  ⚠ Could not clear target: {e}")
            
            # Clear node_modules/.cache
            nm_cache = os.path.join(PROJECT_ROOT, "node_modules", ".cache")
            if os.path.exists(nm_cache):
                try:
                    self.log("  Clearing node_modules/.cache...")
                    shutil.rmtree(nm_cache)
                    cleared += 1
                    self.log("  ✓ Node cache cleared!")
                except Exception as e:
                    self.log(f"  ⚠ Could not clear node cache: {e}")
            
            # Clear .vite cache
            vite_cache = os.path.join(PROJECT_ROOT, "node_modules", ".vite")
            if os.path.exists(vite_cache):
                try:
                    shutil.rmtree(vite_cache)
                    cleared += 1
                except:
                    pass
            
            self.log(f"🔥 Cache clear complete! ({cleared} caches cleared)")
        
        threading.Thread(target=do_clear, daemon=True).start()
    
    def kill_ports(self):
        """Kill processes using common dev server ports (5173-5180, 3000-3010).
        
        Fixes the 'Port 5173 is in use' error that happens when old Vite
        processes are still running in the background.
        """
        self.log("🔌 Killing stuck dev server ports...")
        
        def do_kill():
            import re
            
            # Ports commonly used by Vite, React, etc
            ports_to_check = list(range(5173, 5181)) + list(range(3000, 3011)) + [4173, 8080]
            killed = 0
            
            for port in ports_to_check:
                try:
                    # Find process using this port
                    result = subprocess.run(
                        f'netstat -ano | findstr ":{port}"',
                        shell=True,
                        capture_output=True,
                        text=True,
                        creationflags=subprocess.CREATE_NO_WINDOW
                    )
                    
                    if result.stdout.strip():
                        # Extract PIDs from netstat output
                        lines = result.stdout.strip().split('\n')
                        pids = set()
                        for line in lines:
                            # PID is the last column
                            parts = line.split()
                            if parts:
                                pid = parts[-1]
                                if pid.isdigit() and pid != '0':
                                    pids.add(pid)
                        
                        for pid in pids:
                            try:
                                subprocess.run(
                                    f'taskkill /F /PID {pid}',
                                    shell=True,
                                    capture_output=True,
                                    creationflags=subprocess.CREATE_NO_WINDOW
                                )
                                self.log(f"  ✓ Killed PID {pid} (port {port})")
                                killed += 1
                            except:
                                pass
                except:
                    pass
            
            if killed > 0:
                self.log(f"🔌 Done! Killed {killed} stuck processes.")
            else:
                self.log("🔌 No stuck ports found - all clear!")
        
        threading.Thread(target=do_kill, daemon=True).start()
    
    def close_all_explorers(self):
        """Close all File Explorer windows (without killing explorer.exe)"""
        self.log("📁 Closing Explorer windows...")
        try:
            # Use PowerShell to close Explorer windows via COM
            ps_cmd = '''powershell -Command "(New-Object -ComObject Shell.Application).Windows() | ForEach-Object { $_.Quit() }"'''
            subprocess.run(ps_cmd, shell=True, creationflags=subprocess.CREATE_NO_WINDOW)
            self.log("  ✓ Explorer windows closed!")
        except Exception as e:
            self.log(f"  ⚠ Error: {e}")
    
    def organize_windows(self):
        """Tile all visible windows using Windows API"""
        self.log("🪟 Organizing windows...")
        try:
            # Direct call to user32.dll TileWindows
            # MDITILE_HORIZONTAL = 1, MDITILE_VERTICAL = 0
            user32.TileWindows(None, 1, None, 0, None)
            self.log("  ✓ Windows tiled!")
        except Exception as e:
            self.log(f"  ⚠ Error: {e}")
    
    def minimize_all(self):
        """Minimize all windows (Show Desktop)"""
        self.log("⬇ Minimizing all windows...")
        try:
            # Use Shell.Application to toggle desktop
            ps_cmd = '''powershell -Command "(New-Object -ComObject Shell.Application).MinimizeAll()"'''
            subprocess.run(ps_cmd, shell=True, creationflags=subprocess.CREATE_NO_WINDOW)
            self.log("  ✓ All minimized!")
        except Exception as e:
            self.log(f"  ⚠ Error: {e}")
    
    def open_restore_menu(self):
        """Open a menu to select and restore a previous backup."""
        restore_win = tk.Toplevel(self.root)
        restore_win.title("Restore Backup")
        restore_win.geometry("500x600")
        restore_win.configure(bg="#0f0f0f")
        restore_win.attributes("-topmost", True)
        
        # Header
        tk.Label(restore_win, text="↺ RESTORE POINT", font=("Segoe UI", 14, "bold"), fg="#ff9f00", bg="#0f0f0f").pack(pady=(15, 5))
        tk.Label(restore_win, text="Select a snapshot to roll back to.", font=("Consolas", 9), fg="#888", bg="#0f0f0f").pack(pady=(0, 15))
        
        # Listbox frame
        list_frame = tk.Frame(restore_win, bg="#1a1a1a")
        list_frame.pack(fill="both", expand=True, padx=20, pady=10)
        
        # Scrollbar
        scrollbar = tk.Scrollbar(list_frame)
        scrollbar.pack(side="right", fill="y")
        
        # Listbox
        backup_list = tk.Listbox(
            list_frame,
            bg="#111",
            fg="#ddd",
            font=("Consolas", 10),
            selectbackground="#ff9f00",
            selectforeground="black",
            borderwidth=0,
            highlightthickness=1,
            highlightbackground="#333",
            yscrollcommand=scrollbar.set
        )
        backup_list.pack(side="left", fill="both", expand=True)
        scrollbar.config(command=backup_list.yview)
        
        # Find backups
        backups = []
        for root, dirs, files in os.walk(DEST_ROOT):
            for file in files:
                if file.endswith(".zip"):
                    full_path = os.path.join(root, file)
                    # Get mod time
                    mod_time = os.path.getmtime(full_path)
                    date_str = datetime.fromtimestamp(mod_time).strftime("%Y-%m-%d %H:%M:%S")
                    backups.append((mod_time, full_path, file, date_str))
        
        # Sort by newest first
        backups.sort(key=lambda x: x[0], reverse=True)
        
        for b in backups:
            # Format: [Date] Filename
            backup_list.insert(tk.END, f"[{b[3]}] {b[2]}")
            
        def do_restore():
            selection = backup_list.curselection()
            if not selection:
                return
            
            index = selection[0]
            target_backup = backups[index][1] # full path
            filename = backups[index][2]
            
            # Confirm
            if not tk.messagebox.askyesno("Confirm Restore", f"Are you sure you want to restore:\n{filename}\n\nThis will OVERWRITE your current project files.\nA safety backup will be created first.", parent=restore_win):
                return
            
            restore_win.destroy()
            self.perform_restore(target_backup)
            
        # Restore Button
        restore_btn = tk.Button(
            restore_win,
            text="RESTORE SELECTED SNAPSHOT",
            command=do_restore,
            bg="#3a2a0a", # Dark orangeish
            fg="#ff9f00",
            activebackground="#ff9f00",
            activeforeground="black",
            relief="flat",
            font=("Segoe UI", 10, "bold"),
            cursor="hand2"
        )
        restore_btn.pack(pady=20, ipadx=20, ipady=10)

    def perform_restore(self, backup_path):
        """Execute the restore process safely."""
        self.log(f"↺ INIT RESTORE: {os.path.basename(backup_path)}")
        
        def run():
            # 1. Automatic Safety Backup
            self.log("  Step 1: Creating safety backup...")
            self.status_label.config(text="● PRE-RESTORE...", fg="#ff4757")
            
            # Run a quick synchronous backup
            # We can reuse run_backup logic but we want it blocking and with a specific name
            now = datetime.now()
            date_folder = now.strftime("%b-%d") 
            time_suffix = now.strftime("%I-%M%p") + "_PRE_RESTORE_SAFETY"
            save_path = os.path.join(DEST_ROOT, date_folder)
            if not os.path.exists(save_path): os.makedirs(save_path)
            
            project_name = os.path.basename(PROJECT_ROOT)
            safety_zip = os.path.join(save_path, f"{project_name}_{time_suffix}.zip")
            
            cmd = [
                WINRAR_EXE, "a", "-afzip", "-r", "-ibck", "-dh", "-ep1",
                f"-x{os.path.join(PROJECT_ROOT, 'node_modules')}",
                f"-x{os.path.join(PROJECT_ROOT, '.git')}",
                f"-x{os.path.join(PROJECT_ROOT, 'backups')}",
                # Exclude build artifacts (Root + Crates)
                f"-x{os.path.join(PROJECT_ROOT, 'target')}",
                f"-x{os.path.join(PROJECT_ROOT, 'src-tauri', 'target')}",
                f"-x{os.path.join(PROJECT_ROOT, 'src-tauri', 'gen')}",
                f"-x{os.path.join(PROJECT_ROOT, 'src-bevy', 'target')}",
                f"-x{os.path.join(PROJECT_ROOT, 'crates', 'k-os-engine', 'target')}",
                "-x*.bat", "-x*.zip", "-x*.rar", "-xCargo.lock",
                safety_zip, 
                os.path.join(PROJECT_ROOT, "*")
            ]
            
            try:
                subprocess.run(cmd, creationflags=subprocess.CREATE_NO_WINDOW)
                self.log(f"  ✓ Safety backup created: {os.path.basename(safety_zip)}")
            except Exception as e:
                self.log(f"  ⚠ Safety backup FAILED: {e}")
                self.log("  ABORTING RESTORE TO PROTECT DATA.")
                return

            # 2. Kill Processes
            self.log("  Step 2: Cleaning up processes...")
            if self.tauri_process:
                self.stop_tauri()
            
            # Kill any locks
            os.system(f'taskkill /F /IM "node.exe" /T >nul 2>&1')
            os.system(f'taskkill /F /IM "cargo.exe" /T >nul 2>&1')
            
            # 3. Restore
            self.log("  Step 3: Extracting files...")
            try:
                # 7-Zip/WinRAR extraction
                # WinRAR: x = extract with full paths, -y = assume yes to overwrite
                # -o+ equivalent is default for WinRAR x? No, -o+ is overwrite all
                extract_cmd = [
                    WINRAR_EXE, "x", "-y", "-ibck", 
                    backup_path, 
                    PROJECT_ROOT
                ]
                
                subprocess.run(extract_cmd, creationflags=subprocess.CREATE_NO_WINDOW)
                self.log("  ✓ Files extracted successfully!")
                self.log("↺ RESTORE COMPLETE.")
                self.status_label.config(text="● RESTORED", fg="#00ff9d")
                
            except Exception as e:
                self.log(f"  ⚠ Extraction ERROR: {e}")
                self.status_label.config(text="● ERROR", fg="red")

        threading.Thread(target=run, daemon=True).start()

    def open_settings(self):
        """Open settings window for widget customization"""
        settings = tk.Toplevel(self.root)
        settings.title("Widget Settings")
        settings.geometry("320x280")
        settings.configure(bg="#0f0f0f")
        settings.resizable(False, False)
        settings.attributes("-topmost", True)
        
        # Header
        header = tk.Label(settings, text="⚙ WIDGET SETTINGS", font=("Segoe UI", 12, "bold"), fg=self.accent_color, bg="#0f0f0f")
        header.pack(pady=15)
        
        # Opacity slider
        opacity_frame = tk.Frame(settings, bg="#1a1a1a")
        opacity_frame.pack(fill="x", padx=20, pady=10)
        
        tk.Label(opacity_frame, text="Opacity", font=("Consolas", 9), fg="#888", bg="#1a1a1a").pack(anchor="w", padx=10, pady=(8, 2))
        
        current_opacity = self.widget.attributes("-alpha")
        opacity_var = tk.DoubleVar(value=current_opacity)
        
        def update_opacity(val):
            self.widget.attributes("-alpha", float(val))
        
        opacity_slider = tk.Scale(
            opacity_frame, 
            from_=0.3, 
            to=1.0, 
            resolution=0.05,
            orient="horizontal",
            variable=opacity_var,
            command=update_opacity,
            bg="#1a1a1a",
            fg="#00ff9d",
            highlightthickness=0,
            troughcolor="#0f0f0f",
            activebackground="#00ff9d"
        )
        opacity_slider.pack(fill="x", padx=10, pady=(0, 8))
        
        # Accent color
        color_frame = tk.Frame(settings, bg="#1a1a1a")
        color_frame.pack(fill="x", padx=20, pady=10)
        
        tk.Label(color_frame, text="Accent Color", font=("Consolas", 9), fg="#888", bg="#1a1a1a").pack(anchor="w", padx=10, pady=(8, 2))
        
        colors = [
            ("#00ff9d", "Mint"),
            ("#00bfff", "Cyan"),
            ("#ff00ff", "Magenta"),
            ("#ffff00", "Yellow"),
            ("#ff6b6b", "Red")
        ]
        
        color_btn_frame = tk.Frame(color_frame, bg="#1a1a1a")
        color_btn_frame.pack(padx=10, pady=(0, 8))
        
        def set_accent_color(color):
            self.accent_color = color
            self.widget_btn.config(fg=color)
            header.config(fg=color)
        
        for color, name in colors:
            btn = tk.Label(
                color_btn_frame,
                text="●",
                font=("Arial", 16),
                fg=color,
                bg="#1a1a1a",
                cursor="hand2",
                padx=8
            )
            btn.pack(side="left")
            btn.bind("<Button-1>", lambda e, c=color: set_accent_color(c))
        
        # Background opacity note
        note_frame = tk.Frame(settings, bg="#1a1a1a")
        note_frame.pack(fill="x", padx=20, pady=10)
        
        tk.Label(
            note_frame, 
            text="💡 Drag the widget to reposition it anywhere on screen",
            font=("Consolas", 8),
            fg="#555",
            bg="#1a1a1a",
            wraplength=280
        ).pack(padx=10, pady=10)
        
        # Close button
        close_btn = tk.Button(
            settings,
            text="CLOSE",
            command=settings.destroy,
            bg="#222",
            fg="white",
            activebackground="#333",
            activeforeground="white",
            relief="flat",
            font=("Segoe UI", 9, "bold"),
            cursor="hand2"
        )
        close_btn.pack(pady=10, ipadx=30, ipady=5)

    def log(self, message):
        timestamp = datetime.now().strftime("%H:%M:%S")
        self.log_box.insert(tk.END, f"[{timestamp}] {message}\n")
        self.log_box.see(tk.END)

    def run_backup(self, custom_name=""):
        """Run the backup process immediately - no file watching"""
        try:
            self.status_label.config(text="● ARCHIVING...", fg="#ffff00")
        except:
            pass
        
        self.log("Starting backup...")
        
        now = datetime.now()
        date_folder = now.strftime("%b-%d")
        time_suffix = now.strftime("%I-%M%p")
        
        # Process custom name
        if custom_name and custom_name != "Name...":
            # Sanitize: allow alphanumeric, underscore, hyphen, space
            safe_name = "".join(c for c in custom_name if c.isalnum() or c in (' ', '_', '-')).strip()
            safe_name = safe_name.replace(" ", "_")
            if safe_name:
                time_suffix += f"_{safe_name}"
        
        daily_backup_path = os.path.join(DEST_ROOT, date_folder)
        if not os.path.exists(daily_backup_path): os.makedirs(daily_backup_path)
        
        project_name = os.path.basename(PROJECT_ROOT)
        zip_name = f"{project_name}_{time_suffix}.zip"
        final_output = os.path.join(daily_backup_path, zip_name)

        cmd = [
            WINRAR_EXE, "a", "-afzip", "-r", "-ibck", "-dh", "-ep1",
            # Exclude heavy directories
            f"-x{os.path.join(PROJECT_ROOT, 'node_modules')}",
            f"-x{os.path.join(PROJECT_ROOT, '.git')}",
            f"-x{os.path.join(PROJECT_ROOT, 'backups')}",
            # Exclude build artifacts (Root + Crates)
            f"-x{os.path.join(PROJECT_ROOT, 'target')}",
            f"-x{os.path.join(PROJECT_ROOT, 'src-tauri', 'target')}",
            f"-x{os.path.join(PROJECT_ROOT, 'src-tauri', 'gen')}",
            f"-x{os.path.join(PROJECT_ROOT, 'src-bevy', 'target')}",
            f"-x{os.path.join(PROJECT_ROOT, 'crates', 'k-os-engine', 'target')}",
            # Exclude file types
            "-x*.bat", "-x*.log", "-x.DS_Store",
            "-x*.zip", "-x*.rar", "-x*.7z", "-x*.kipp",
            "-x*.exe", "-x*.dll", "-x*.pdb",
            "-xCargo.lock", "-xpackage-lock.json",
            final_output, 
            os.path.join(PROJECT_ROOT, "*") 
        ]
        
        try:
            subprocess.run(cmd, creationflags=subprocess.CREATE_NO_WINDOW)
            self.log(f"✓ Saved: {zip_name}")
            time_str = now.strftime('%I:%M %p')
            self.last_run_label.config(text=f"LAST SNAPSHOT: {time_str}")
            self.update_widget_time(time_str)
            
            # Reset input field on main thread if needed (optional, keeping it for now so user remembers what they backed up)
            # self.root.after(0, lambda: self.backup_name_var.set("Name..."))
            
        except Exception as e:
            self.log(f"Error: {e}")
        
        try:
            self.status_label.config(text="● TITAN V2 READY", fg=self.accent_color)
        except:
            pass

    def force_backup(self):
        # Get name from UI thread before starting worker thread
        name = self.backup_name_var.get()
        self.log("Manual backup triggered...")
        threading.Thread(target=self.run_backup, args=(name,), daemon=True).start()

    # --- CLIPBOARD SNIPPET METHODS ---
    def create_new_snippet(self):
        """Create a new text snippet file and open in notepad"""
        from tkinter import simpledialog
        name = simpledialog.askstring("New Snippet", "Enter snippet name (no extension):", parent=self.widget)
        if not name or not name.strip():
            return
        name = name.strip().replace(" ", "_")
        filename = f"{name}.txt"
        filepath = os.path.join(self.clipboard_folder, filename)
        try:
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write("")
            os.startfile(filepath)
            self.log(f"📝 Created snippet: {filename}")
            self.root.after(500, self.refresh_clipboard_snippets)
        except Exception as e:
            self.log(f"Error creating snippet: {e}")
    
    def refresh_clipboard_snippets(self):
        """Refresh the list of clipboard snippets"""
        for widget in self.clipboard_scroll_frame.winfo_children():
            widget.destroy()
        try:
            files = [f for f in os.listdir(self.clipboard_folder) if f.endswith('.txt')]
            files.sort()
            if not files:
                empty_label = tk.Label(self.clipboard_scroll_frame, text="No snippets yet.\nClick + NEW SNIPPET to create one!", font=("Segoe UI", 9), fg="#666", bg="#111", pady=20)
                empty_label.pack()
                return
            for filename in files:
                filepath = os.path.join(self.clipboard_folder, filename)
                display_name = filename[:-4]
                row = tk.Frame(self.clipboard_scroll_frame, bg="#111")
                row.pack(fill="x", pady=1)
                copy_btn = tk.Label(row, text=f"📋 {display_name}", font=("Segoe UI", 8), fg="#ffd54f", bg="#1a1a1a", cursor="hand2", pady=5, anchor="w", padx=10)
                copy_btn.pack(side="left", fill="x", expand=True)
                copy_btn.bind("<Button-1>", lambda e, fp=filepath: self.copy_snippet_to_clipboard(fp))
                copy_btn.bind("<Enter>", lambda e, btn=copy_btn: btn.config(bg="#252525"))
                copy_btn.bind("<Leave>", lambda e, btn=copy_btn: btn.config(bg="#1a1a1a"))
                edit_btn = tk.Label(row, text="✏", font=("Arial", 9), fg="#888", bg="#1a1a1a", cursor="hand2", padx=8, pady=5)
                edit_btn.pack(side="right")
                edit_btn.bind("<Button-1>", lambda e, fp=filepath: os.startfile(fp))
                edit_btn.bind("<Enter>", lambda e, btn=edit_btn: btn.config(fg="#fff"))
                edit_btn.bind("<Leave>", lambda e, btn=edit_btn: btn.config(fg="#888"))
        except Exception as e:
            self.log(f"Error refreshing snippets: {e}")
    
    def copy_snippet_to_clipboard(self, filepath):
        """Copy the content of a snippet file to clipboard"""
        try:
            with open(filepath, 'r', encoding='utf-8') as f:
                content = f.read()
            self.root.clipboard_clear()
            self.root.clipboard_append(content)
            self.root.update()
            filename = os.path.basename(filepath)
            self.log(f"📋 Copied: {filename}")
        except Exception as e:
            self.log(f"Error copying snippet: {e}")
    
    # --- SCREENSHOT GALLERY METHODS ---
    def refresh_screenshots(self):
        """Refresh the list of screenshots from the folder."""
        for widget in self.screens_scroll_frame.winfo_children():
            widget.destroy()
        
        try:
            # Get image files
            valid_extensions = ('.png', '.jpg', '.jpeg', '.gif', '.bmp', '.webp')
            files = [f for f in os.listdir(self.screenshots_folder) 
                     if f.lower().endswith(valid_extensions)]
            
            # Sort by modification time (newest first)
            files.sort(key=lambda x: os.path.getmtime(os.path.join(self.screenshots_folder, x)), reverse=True)
            
            if not files:
                empty_label = tk.Label(
                    self.screens_scroll_frame, 
                    text="No screenshots yet.\nTake a screenshot (Win+Shift+S)\nand save to SCREENSHOTS folder!", 
                    font=("Segoe UI", 9), fg="#666", bg="#111", pady=20
                )
                empty_label.pack()
                return
            
            # Show only first 10 to keep it compact
            for filename in files[:10]:
                filepath = os.path.join(self.screenshots_folder, filename)
                display_name = filename[:25] + "..." if len(filename) > 28 else filename
                
                row = tk.Frame(self.screens_scroll_frame, bg="#111")
                row.pack(fill="x", pady=1)
                
                # Copy button (copies image to clipboard)
                copy_btn = tk.Label(
                    row, text=f"📷 {display_name}", 
                    font=("Segoe UI", 8), fg="#4fc3f7", bg="#1a1a1a", 
                    cursor="hand2", pady=5, anchor="w", padx=10
                )
                copy_btn.pack(side="left", fill="x", expand=True)
                copy_btn.bind("<Button-1>", lambda e, fp=filepath: self.copy_screenshot_to_clipboard(fp))
                copy_btn.bind("<Enter>", lambda e, btn=copy_btn: btn.config(bg="#252525"))
                copy_btn.bind("<Leave>", lambda e, btn=copy_btn: btn.config(bg="#1a1a1a"))
                
                # View button (opens in default viewer)
                view_btn = tk.Label(
                    row, text="👁", font=("Arial", 9), fg="#888", 
                    bg="#1a1a1a", cursor="hand2", padx=8, pady=5
                )
                view_btn.pack(side="right")
                view_btn.bind("<Button-1>", lambda e, fp=filepath: os.startfile(fp))
                view_btn.bind("<Enter>", lambda e, btn=view_btn: btn.config(fg="#fff"))
                view_btn.bind("<Leave>", lambda e, btn=view_btn: btn.config(fg="#888"))
            
            # Show count if there are more
            if len(files) > 10:
                more_label = tk.Label(
                    self.screens_scroll_frame, 
                    text=f"... and {len(files) - 10} more (click OPEN FOLDER to see all)", 
                    font=("Consolas", 7), fg="#555", bg="#111", pady=5
                )
                more_label.pack()
                
        except Exception as e:
            self.log(f"Error refreshing screenshots: {e}")
    
    def copy_screenshot_to_clipboard(self, filepath):
        """Copy a screenshot image to the clipboard."""
        try:
            # Use PowerShell to copy image to clipboard (Windows)
            ps_script = f'''
            Add-Type -AssemblyName System.Windows.Forms
            $image = [System.Drawing.Image]::FromFile("{filepath}")
            [System.Windows.Forms.Clipboard]::SetImage($image)
            '''
            
            subprocess.run(
                ["powershell", "-Command", ps_script],
                creationflags=subprocess.CREATE_NO_WINDOW,
                capture_output=True
            )
            
            filename = os.path.basename(filepath)
            self.log(f"📷 Copied to clipboard: {filename}")
            
        except Exception as e:
            self.log(f"Error copying screenshot: {e}")
    
    # --- TODO SYSTEM METHODS ---
    def load_todos(self):
        """Load todos from JSON file."""
        try:
            if os.path.exists(self.todo_file):
                with open(self.todo_file, 'r', encoding='utf-8') as f:
                    import json
                    return json.load(f)
        except Exception as e:
            self.log(f"Error loading todos: {e}")
        
        # Return default structure
        return {"TODO": [], "BUGS": [], "FINISH": []}
    
    def save_todos(self):
        """Save todos to JSON file."""
        try:
            import json
            with open(self.todo_file, 'w', encoding='utf-8') as f:
                json.dump(self.todos, f, indent=2)
        except Exception as e:
            self.log(f"Error saving todos: {e}")
    
    def refresh_todo_list(self):
        """Refresh the todo list display for current category."""
        for widget in self.todo_scroll_frame.winfo_children():
            widget.destroy()
        
        category = self.current_todo_category
        items = self.todos.get(category, [])
        
        if not items:
            empty_label = tk.Label(
                self.todo_scroll_frame,
                text=f"No {category.lower()} items yet.\nClick + to add one!",
                font=("Segoe UI", 9), fg="#666", bg="#111", pady=20
            )
            empty_label.pack()
            return
        
        # Category colors
        cat_colors = {"TODO": "#ff7043", "BUGS": "#e57373", "FINISH": "#81c784"}
        color = cat_colors.get(category, "#fff")
        
        for i, item in enumerate(items):
            row = tk.Frame(self.todo_scroll_frame, bg="#111")
            row.pack(fill="x", pady=1)
            
            # Checkbox
            is_done = item.get("done", False)
            check_text = "☑" if is_done else "☐"
            check_color = "#555" if is_done else color
            
            check_btn = tk.Label(
                row, text=check_text, font=("Arial", 11),
                fg=check_color, bg="#1a1a1a", cursor="hand2", padx=8, pady=4
            )
            check_btn.pack(side="left")
            check_btn.bind("<Button-1>", lambda e, idx=i: self.toggle_todo(idx))
            
            # Task text
            text_color = "#555" if is_done else "#ddd"
            task_text = item.get("text", "")[:40]
            if len(item.get("text", "")) > 40:
                task_text += "..."
            
            text_label = tk.Label(
                row, text=task_text, font=("Segoe UI", 8),
                fg=text_color, bg="#1a1a1a", cursor="hand2",
                pady=4, anchor="w", padx=5
            )
            text_label.pack(side="left", fill="x", expand=True)
            text_label.bind("<Button-1>", lambda e, idx=i: self.toggle_todo(idx))
            
            # Delete button
            del_btn = tk.Label(
                row, text="✕", font=("Arial", 9),
                fg="#555", bg="#1a1a1a", cursor="hand2", padx=8, pady=4
            )
            del_btn.pack(side="right")
            del_btn.bind("<Button-1>", lambda e, idx=i: self.delete_todo(idx))
            del_btn.bind("<Enter>", lambda e, b=del_btn: b.config(fg="#e57373"))
            del_btn.bind("<Leave>", lambda e, b=del_btn: b.config(fg="#555"))
    
    def add_todo_item(self):
        """Add a new todo item to current category."""
        from tkinter import simpledialog
        
        category = self.current_todo_category
        text = simpledialog.askstring(
            f"New {category} Item",
            f"Enter {category.lower()} description:",
            parent=self.widget
        )
        
        if not text or not text.strip():
            return
        
        if category not in self.todos:
            self.todos[category] = []
        
        self.todos[category].append({
            "text": text.strip(),
            "done": False,
            "created": datetime.now().isoformat()
        })
        
        self.save_todos()
        self.refresh_todo_list()
        self.log(f"✓ Added {category}: {text[:30]}...")
    
    def toggle_todo(self, index):
        """Toggle todo item completion status."""
        category = self.current_todo_category
        if category in self.todos and 0 <= index < len(self.todos[category]):
            self.todos[category][index]["done"] = not self.todos[category][index]["done"]
            self.save_todos()
            self.refresh_todo_list()
    
    def delete_todo(self, index):
        """Delete a todo item."""
        category = self.current_todo_category
        if category in self.todos and 0 <= index < len(self.todos[category]):
            item = self.todos[category].pop(index)
            self.save_todos()
            self.refresh_todo_list()
            self.log(f"✕ Deleted: {item.get('text', '')[:20]}...")

if __name__ == "__main__":
    root = tk.Tk()
    app = TitanBackupApp(root)
    root.mainloop()
