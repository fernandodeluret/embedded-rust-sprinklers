// Embed the JS file at compile time
const SYNC_TIME: &str = include_str!("html_scripts/syncTime.js");

pub fn get_root_html(server_time: &str) -> String {
    let html_template = format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Sprinklers</title>
<style>
    body {{ font-family: system-ui; background: #1a1a2e; color: #eee; 
           display: flex; flex-direction: column; align-items: center; 
           padding: 20px; margin: 0; min-height: 100vh; }}
    button {{ padding: 12px 24px; font-size: 16px; background: #4ade80; 
              color: #1a1a2e; border: none; border-radius: 8px; cursor: pointer; margin: 5px; }}
    button:hover {{ background: #22c55e; }}
    .time-display {{ font-size: 20px; margin: 10px; }}
    .aspersor {{ background: #2a2a4e; padding: 15px; margin: 10px; border-radius: 8px; 
                 display: flex; align-items: center; gap: 15px; min-width: 300px; }}
    .aspersor.on {{ border-left: 4px solid #4ade80; }}
    .aspersor.off {{ border-left: 4px solid #666; }}
    .name {{ flex: 1; font-weight: bold; }}
    #manual-mode {{ font-size: 18px; margin: 15px; padding: 10px; background: #2a2a4e; border-radius: 8px; }}
    h2 {{ margin-top: 30px; }}
    .aspersor {{ background: #2a2a4e; padding: 15px; margin: 10px; border-radius: 8px; 
                  display: flex; flex-direction: column; gap: 10px; min-width: 300px; }}
    .aspersor-header {{ display: flex; justify-content: space-between; align-items: center; }}
    .schedule {{ color: #aaa; font-size: 14px; }}
    .mode-btn {{ padding: 15px 25px; font-size: 18px; border: none; border-radius: 8px; 
                cursor: pointer; margin: 15px; transition: all 0.2s; }}
    .mode-btn.manual {{ background: #f59e0b; color: #1a1a2e; }}
    .mode-btn.auto {{ background: #3b82f6; color: white; }}
    .mode-btn:hover {{ opacity: 0.8; }}
    .mode-btn small {{ display: block; font-size: 12px; opacity: 0.7; margin-top: 5px; }}
    .edit-row {{ display: flex; gap: 10px; align-items: center; flex-wrap: wrap; }}
    .edit-row label {{ font-size: 14px; color: #aaa; }}
    .edit-row input {{ padding: 5px; border-radius: 4px; border: 1px solid #444; 
                        background: #1a1a2e; color: #eee; }}
    .edit-row input[type="time"] {{ width: 100px; }}
    .edit-row input[type="number"] {{ width: 70px; }}
    .save-btn {{ padding: 5px 10px; font-size: 14px; background: #3b82f6; }}

    @import url('https://fonts.googleapis.com/css2?family=Orbitron:wght@700&display=swap');

    .clock-container {{
      text-align: center;
      margin: 30px 0;
      padding: 25px;
      background: #0a0a0f;
      border-radius: 15px;
      box-shadow: 
        0 0 20px rgba(0, 255, 136, 0.2),
        inset 0 0 30px rgba(0, 0, 0, 0.8);
      border: 2px solid #1a1a2e;
    }}
    .clock-display {{
      font-family: 'Courier New', 'Lucida Console', Monaco, monospace;
      font-size: clamp(28px, 8vw, 56px);  /* Responsive: min 28px, scales with screen, max 56px */
      font-weight: bold;
      color: #00ff88;
      text-shadow: 
        0 0 10px #00ff88,
        0 0 20px #00ff88,
        0 0 40px #00ff8855;
      letter-spacing: clamp(2px, 1vw, 8px);  /* Also scale letter spacing */
      white-space: nowrap;  /* Prevent line wrap */
    }}
    .clock-digit {{
      display: inline-block;
      min-width: clamp(35px, 9vw, 70px);  /* Scale digit width */
    }}
    .clock-digit.seconds {{
      color: #00ccff;
      text-shadow: 
        0 0 10px #00ccff,
        0 0 20px #00ccff;
    }}
    .clock-colon {{
      margin: 0 clamp(2px, 0.5vw, 5px);  /* Scale colon spacing */
      transition: opacity 0.2s;
    }}
    .clock-date {{
      font-family: 'Orbitron', monospace;
      font-size: 20px;
      color: #666;
      margin-top: 10px;
      letter-spacing: 3px;
    }}
    #sync-status {{
      margin-top: 15px;
      font-family: 'Orbitron', monospace;
      font-size: 12px;
      letter-spacing: 2px;
    }}
    .synced {{ color: #00ff88; }}
    .not-synced {{ color: #ff4444; }}

    h1 {{
      font-size: clamp(18px, 5vw, 32px);  /* Scales with screen */
      white-space: nowrap;  /* Force single line */
      margin: 10px 0;
    }}

    .note {{
      font-size: clamp(10px, 2.5vw, 14px);
      color: #f59e0b;
      background: rgba(245, 158, 11, 0.1);
      padding: 8px 12px;
      border-radius: 6px;
      border-left: 3px solid #f59e0b;
      margin: 15px 0;
      white-space: nowrap;
    }}
    .mode-label {{
      padding: 2px 8px;
      border-radius: 4px;
      font-weight: bold;
      font-size: 0.9em;
    }}
    .mode-label.manual {{
      background: #f59e0b;
      color: #1a1a2e;
    }}
    .mode-label.auto {{
      background: #3b82f6;
      color: white;
    }}
</style>
</head>
<body>
<h1>🌱 Control de Aspersores</h1>

<div class="clock-container">
  <div class="clock-display">
    <span class="clock-digit" id="clock-hours">00</span>
    <span class="clock-colon">:</span>
    <span class="clock-digit" id="clock-minutes">00</span>
    <span class="clock-colon">:</span>
    <span class="clock-digit seconds" id="clock-seconds">00</span>
  </div>
  <div class="clock-date" id="clock-date">00/00/0000</div>
  <div id="sync-status"></div>
</div>

<div class="time-display" id="client-time"></div>
<div id="sync-status"></div>
<button onclick="syncTime()">Sync Time</button>
<div id="status"></div>

<button id="manual-mode" onclick="toggleManualMode()"></button>

<p class="note">⚠️ Manual: <span class="mode-label manual">Manual</span> → on/off → <span class="mode-label auto">Auto</span></p>

<h2>Aspersores</h2>
<div id="aspersores">Loading...</div>

<script>
    const SERVER_TIME = "{server_time}";
"#,
        server_time = server_time,
    );

    format!(
        "{}{}\n    </script>\n</body>\n</html>",
        html_template, SYNC_TIME
    )
}
