"""
EQOA Enhanced Client - Desktop Launcher & Mod Manager
Provides a graphical interface to manage mods, toggle QoL/UI features,
and launch the native Rust client with or without the live combat parser.
"""

import os
import sys
import json
import subprocess
import tkinter as tk
from tkinter import ttk, messagebox

# Resolve base directories
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
MODS_DIR = os.path.join(SCRIPT_DIR, "mods")
EXE_PATH = os.path.join(SCRIPT_DIR, "eqoa.exe")
PARSER_DIR = r"C:\Users\hocka\Github Projects\EQOA Parser"
PARSER_APP = os.path.join(PARSER_DIR, "eqoa_parser", "app.py")

# AppData configuration path for client.opt
APPDATA_DIR = os.path.join(os.environ.get("APPDATA", ""), "eqoa-rs")
CLIENT_OPT = os.path.join(APPDATA_DIR, "client.opt")


def read_client_options():
    options = {
        "target_hp_percent": True,
        "xp_details": True,
        "hud_numbers": True,
        "minimap": True,
        "stats_box": False,
        "quest_tracker": True,
        "quest_markers": True,
        "mana_bars": True,
    }
    if os.path.isfile(CLIENT_OPT):
        try:
            with open(CLIENT_OPT, "r", encoding="utf-8") as f:
                for line in f:
                    if "=" in line:
                        k, v = line.strip().split("=", 1)
                        if k in options:
                            options[k] = (v.lower() != "off")
        except Exception:
            pass
    return options


def save_client_options(options):
    os.makedirs(APPDATA_DIR, exist_ok=True)
    existing_lines = []
    if os.path.isfile(CLIENT_OPT):
        try:
            with open(CLIENT_OPT, "r", encoding="utf-8") as f:
                existing_lines = f.readlines()
        except Exception:
            pass

    key_found = set()
    new_lines = []
    for line in existing_lines:
        if "=" in line:
            k = line.split("=", 1)[0].strip()
            if k in options:
                new_lines.append(f"{k}={'on' if options[k] else 'off'}\n")
                key_found.add(k)
                continue
        new_lines.append(line)

    for k, v in options.items():
        if k not in key_found:
            new_lines.append(f"{k}={'on' if v else 'off'}\n")

    try:
        with open(CLIENT_OPT, "w", encoding="utf-8") as f:
            f.writelines(new_lines)
    except Exception as e:
        messagebox.showerror("Error", f"Failed to save settings: {e}")


def list_discovered_mods():
    mods = []
    if not os.path.isdir(MODS_DIR):
        os.makedirs(MODS_DIR, exist_ok=True)

    for item in os.listdir(MODS_DIR):
        item_path = os.path.join(MODS_DIR, item)
        if os.path.isdir(item_path):
            manifest_file = os.path.join(item_path, "mod.json")
            mod_data = {
                "id": item,
                "name": item.replace("_", " ").title(),
                "version": "1.0.0",
                "author": "Community",
                "description": "User dropped mod package",
                "enabled": True,
            }
            if os.path.isfile(manifest_file):
                try:
                    with open(manifest_file, "r", encoding="utf-8") as f:
                        data = json.load(f)
                        mod_data.update(data)
                except Exception:
                    pass
            mods.append(mod_data)
    return mods


class LauncherApp(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("EverQuest Online Adventures - Enhanced Launcher")
        self.geometry("640x660")
        self.resizable(False, False)
        self.configure(bg="#1a1c23")

        self.options = read_client_options()
        self.mods = list_discovered_mods()

        self.style = ttk.Style(self)
        self.style.theme_use("clam")
        self.style.configure(".", background="#1a1c23", foreground="#e1e1e6")
        self.style.configure("TLabel", background="#1a1c23", foreground="#e1e1e6", font=("Segoe UI", 10))
        self.style.configure("Header.TLabel", font=("Segoe UI", 16, "bold"), foreground="#ffd700")
        self.style.configure("SubHeader.TLabel", font=("Segoe UI", 10), foreground="#9999aa")
        self.style.configure("Section.TLabel", font=("Segoe UI", 11, "bold"), foreground="#58a6ff")
        self.style.configure("TCheckbutton", background="#1a1c23", foreground="#e1e1e6", font=("Segoe UI", 10))

        self.create_widgets()

    def create_widgets(self):
        # Header banner
        header_frame = tk.Frame(self, bg="#1a1c23", pady=10)
        header_frame.pack(fill="x", padx=20)

        lbl_title = ttk.Label(header_frame, text="EverQuest Online Adventures", style="Header.TLabel")
        lbl_title.pack(anchor="w")

        lbl_sub = ttk.Label(header_frame, text="Enhanced Native Client & Mod Manager", style="SubHeader.TLabel")
        lbl_sub.pack(anchor="w")

        # Status row
        exe_status = "Ready (eqoa.exe detected)" if os.path.isfile(EXE_PATH) else "Warning: eqoa.exe not found"
        status_color = "#3fb950" if os.path.isfile(EXE_PATH) else "#f85149"
        lbl_status = tk.Label(header_frame, text=f"Client Status: {exe_status}", bg="#1a1c23", fg=status_color, font=("Segoe UI", 9))
        lbl_status.pack(anchor="w", pady=(2, 0))

        # Separator
        ttk.Separator(self, orient="horizontal").pack(fill="x", padx=20, pady=6)

        # UI & QoL Features Frame
        ui_frame = tk.LabelFrame(self, text=" Enhanced UI & Quality of Life Features ", bg="#1a1c23", fg="#58a6ff", font=("Segoe UI", 11, "bold"), padx=15, pady=10)
        ui_frame.pack(fill="x", padx=20, pady=5)

        self.var_target_hp = tk.BooleanVar(value=self.options.get("target_hp_percent", True))
        chk_hp = tk.Checkbutton(ui_frame, text="Target Health Percentage (show exact % on target bar)", variable=self.var_target_hp, bg="#1a1c23", fg="#ffffff", selectcolor="#2d333b", activebackground="#1a1c23", activeforeground="#ffffff", font=("Segoe UI", 9))
        chk_hp.pack(anchor="w", pady=2)

        self.var_xp_details = tk.BooleanVar(value=self.options.get("xp_details", True))
        chk_xp = tk.Checkbutton(ui_frame, text="Detailed XP Progress (numerical values & % on XP bar)", variable=self.var_xp_details, bg="#1a1c23", fg="#ffffff", selectcolor="#2d333b", activebackground="#1a1c23", activeforeground="#ffffff", font=("Segoe UI", 9))
        chk_xp.pack(anchor="w", pady=2)

        self.var_hud_numbers = tk.BooleanVar(value=self.options.get("hud_numbers", True))
        chk_nums = tk.Checkbutton(ui_frame, text="HUD Numbers (numerical player Health & Power overlays)", variable=self.var_hud_numbers, bg="#1a1c23", fg="#ffffff", selectcolor="#2d333b", activebackground="#1a1c23", activeforeground="#ffffff", font=("Segoe UI", 9))
        chk_nums.pack(anchor="w", pady=2)

        self.var_minimap = tk.BooleanVar(value=self.options.get("minimap", True))
        chk_mini = tk.Checkbutton(ui_frame, text="Minimap (live radar under ability bars, toggle with N)", variable=self.var_minimap, bg="#1a1c23", fg="#ffffff", selectcolor="#2d333b", activebackground="#1a1c23", activeforeground="#ffffff", font=("Segoe UI", 9))
        chk_mini.pack(anchor="w", pady=2)

        self.var_stats_box = tk.BooleanVar(value=self.options.get("stats_box", False))
        chk_stat = tk.Checkbutton(ui_frame, text="On-Screen Stats Box (AC, resists, base stats, toggle with B)", variable=self.var_stats_box, bg="#1a1c23", fg="#ffffff", selectcolor="#2d333b", activebackground="#1a1c23", activeforeground="#ffffff", font=("Segoe UI", 9))
        chk_stat.pack(anchor="w", pady=2)

        self.var_tracker = tk.BooleanVar(value=self.options.get("quest_tracker", True))
        chk_track = tk.Checkbutton(ui_frame, text="Quest Tracker & Map Markers (objective count under minimap)", variable=self.var_tracker, bg="#1a1c23", fg="#ffffff", selectcolor="#2d333b", activebackground="#1a1c23", activeforeground="#ffffff", font=("Segoe UI", 9))
        chk_track.pack(anchor="w", pady=2)

        # Installed Mods Frame
        mods_frame = tk.LabelFrame(self, text=" Installed Mods (mods/ folder) ", bg="#1a1c23", fg="#58a6ff", font=("Segoe UI", 11, "bold"), padx=15, pady=10)
        mods_frame.pack(fill="both", expand=True, padx=20, pady=8)

        # Mod listbox / items
        for mod in self.mods:
            mod_row = tk.Frame(mods_frame, bg="#1a1c23")
            mod_row.pack(fill="x", pady=2)

            mod_lbl = tk.Label(mod_row, text=f"• {mod['name']} (v{mod.get('version', '1.0')})", bg="#1a1c23", fg="#388bfd", font=("Segoe UI", 10, "bold"))
            mod_lbl.pack(anchor="w")

            desc_lbl = tk.Label(mod_row, text=f"   {mod.get('description', '')}", bg="#1a1c23", fg="#8b949e", font=("Segoe UI", 9), wraplength=560, justify="left")
            desc_lbl.pack(anchor="w")

        btn_folder = tk.Button(mods_frame, text="📁 Open 'mods' Folder", command=self.open_mods_folder, bg="#21262d", fg="#c9d1d9", relief="flat", font=("Segoe UI", 9), padx=10, pady=4)
        btn_folder.pack(anchor="e", pady=(8, 0))

        # Bottom Action Bar
        bottom_frame = tk.Frame(self, bg="#16181d", pady=15)
        bottom_frame.pack(fill="x", side="bottom")

        btn_play = tk.Button(bottom_frame, text="▶  PLAY EQOA", command=self.launch_game, bg="#238636", fg="#ffffff", activebackground="#2ea043", font=("Segoe UI", 11, "bold"), padx=20, pady=8, relief="flat", cursor="hand2")
        btn_play.pack(side="right", padx=20)

        btn_parser = tk.Button(bottom_frame, text="⚔  Play with Combat Parser", command=self.launch_with_parser, bg="#1f6feb", fg="#ffffff", activebackground="#388bfd", font=("Segoe UI", 10, "bold"), padx=15, pady=8, relief="flat", cursor="hand2")
        btn_parser.pack(side="right", padx=5)

        lbl_hint = tk.Label(bottom_frame, text="Press Esc in-game for Options -> Extended", bg="#16181d", fg="#8b949e", font=("Segoe UI", 8))
        lbl_hint.pack(side="left", padx=20)

    def save_current_options(self):
        self.options["target_hp_percent"] = self.var_target_hp.get()
        self.options["xp_details"] = self.var_xp_details.get()
        self.options["hud_numbers"] = self.var_hud_numbers.get()
        self.options["minimap"] = self.var_minimap.get()
        self.options["stats_box"] = self.var_stats_box.get()
        self.options["quest_tracker"] = self.var_tracker.get()
        save_client_options(self.options)

    def open_mods_folder(self):
        if not os.path.isdir(MODS_DIR):
            os.makedirs(MODS_DIR, exist_ok=True)
        os.startfile(MODS_DIR)

    def launch_game(self):
        self.save_current_options()
        if not os.path.isfile(EXE_PATH):
            messagebox.showerror("Error", f"Could not find eqoa.exe in:\n{SCRIPT_DIR}")
            return
        subprocess.Popen([EXE_PATH], cwd=SCRIPT_DIR)
        self.destroy()

    def launch_with_parser(self):
        self.save_current_options()
        if not os.path.isfile(EXE_PATH):
            messagebox.showerror("Error", f"Could not find eqoa.exe in:\n{SCRIPT_DIR}")
            return

        # Start parser in companion mode if script exists
        if os.path.isfile(PARSER_APP):
            subprocess.Popen([sys.executable, PARSER_APP, "--client"], cwd=PARSER_DIR)

        # Start game client
        subprocess.Popen([EXE_PATH], cwd=SCRIPT_DIR)
        self.destroy()


if __name__ == "__main__":
    app = LauncherApp()
    app.mainloop()
