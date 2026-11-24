pub const HTML_INDEX: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>TouchpadFlow Control Center</title>
<style>
:root {
  --bg: #0b0f19;
  --panel: #131b2e;
  --border: #1f2e4d;
  --text: #e2e8f0;
  --muted: #8494b0;
  --accent: #38bdf8;
  --accent-vol: #f59e0b;
  --accent-bright: #38bdf8;
  --success: #10b981;
  --danger: #ef4444;
}
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  background: var(--bg);
  color: var(--text);
  line-height: 1.5;
  padding: 24px;
}
.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border);
}
.title-group h1 { font-size: 20px; font-weight: 700; letter-spacing: -0.5px; }
.title-group p { font-size: 13px; color: var(--muted); }
.status-pill {
  display: flex;
  align-items: center;
  gap: 8px;
  background: var(--panel);
  border: 1px solid var(--border);
  padding: 6px 14px;
  border-radius: 9999px;
  font-size: 13px;
  font-weight: 600;
}
.dot { width: 8px; height: 8px; border-radius: 50%; background: var(--success); }
.dot.stopped { background: var(--danger); }
.grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}
@media (max-width: 900px) { .grid { grid-template-columns: 1fr; } }
.card {
  background: var(--panel);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 18px;
}
.card h2 {
  font-size: 14px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--muted);
  margin-bottom: 14px;
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.touchpad-box {
  width: 100%;
  height: 240px;
  background: #090d16;
  border: 2px solid var(--border);
  border-radius: 12px;
  position: relative;
  overflow: hidden;
  cursor: crosshair;
}
.zone {
  position: absolute;
  top: 0;
  bottom: 0;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.5px;
  pointer-events: none;
  transition: width 0.15s ease;
}
.zone-left {
  left: 0;
  background: rgba(56, 189, 248, 0.08);
  border-right: 2px dashed rgba(56, 189, 248, 0.4);
  color: var(--accent-bright);
}
.zone-right {
  right: 0;
  background: rgba(245, 158, 11, 0.08);
  border-left: 2px dashed rgba(245, 158, 11, 0.4);
  color: var(--accent-vol);
}
.zone-center {
  position: absolute;
  top: 0;
  bottom: 0;
  display: flex;
  justify-content: center;
  align-items: center;
  color: #3b4e70;
  font-size: 12px;
}
.finger-dot {
  position: absolute;
  width: 24px;
  height: 24px;
  margin-left: -12px;
  margin-top: -12px;
  background: rgba(255, 255, 255, 0.9);
  border: 2px solid var(--accent);
  border-radius: 50%;
  box-shadow: 0 0 12px rgba(56, 189, 248, 0.6);
  pointer-events: none;
  display: none;
}
.controls-group { display: flex; flex-direction: column; gap: 14px; }
.control-item { display: flex; flex-direction: column; gap: 6px; }
.control-label {
  display: flex;
  justify-content: space-between;
  font-size: 13px;
  font-weight: 500;
}
.control-label span:last-child { color: var(--accent); font-family: monospace; }
input[type="range"] {
  -webkit-appearance: none;
  width: 100%;
  height: 6px;
  background: #1c273e;
  border-radius: 3px;
  outline: none;
}
input[type="range"]::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 16px;
  height: 16px;
  background: var(--accent);
  border-radius: 50%;
  cursor: pointer;
}
.btn-group { display: flex; gap: 8px; flex-wrap: wrap; }
button {
  background: #1c273e;
  color: var(--text);
  border: 1px solid var(--border);
  padding: 7px 14px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.1s ease;
}
button:hover { background: #273756; border-color: #3b4e70; }
button.primary { background: var(--accent); color: #000; border-color: var(--accent); font-weight: 600; }
button.primary:hover { background: #7dd3fc; }
button.danger { border-color: var(--danger); color: var(--danger); }
button.danger:hover { background: rgba(239, 68, 68, 0.15); }
.test-bench {
  display: flex;
  gap: 16px;
  margin-top: 14px;
  padding: 12px;
  background: #090d16;
  border-radius: 8px;
}
.meter { flex: 1; display: flex; flex-direction: column; gap: 6px; }
.meter-label { font-size: 11px; text-transform: uppercase; color: var(--muted); font-weight: 600; }
.meter-bar {
  height: 12px;
  background: #1c273e;
  border-radius: 6px;
  overflow: hidden;
  position: relative;
}
.meter-fill { height: 100%; transition: width 0.05s ease; width: 50%; }
.meter-vol .meter-fill { background: var(--accent-vol); }
.meter-bright .meter-fill { background: var(--accent-bright); }
.table { width: 100%; border-collapse: collapse; font-size: 13px; margin-top: 8px; }
.table th, .table td { text-align: left; padding: 8px 10px; border-bottom: 1px solid var(--border); }
.table th { color: var(--muted); font-weight: 600; font-size: 11px; text-transform: uppercase; }
select {
  background: #090d16;
  border: 1px solid var(--border);
  color: var(--text);
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 12px;
}
</style>
</head>
<body>

<div class="header">
  <div class="title-group">
    <h1>TouchpadFlow Control Center</h1>
    <p>Hardware Multi-Touch Gesture Calibration & Control</p>
  </div>
  <div class="status-pill">
    <div id="statusDot" class="dot"></div>
    <span id="statusText">Active Engine</span>
  </div>
</div>

<div class="grid">

  <!-- 1. Live Preview & Gesture Testing -->
  <div class="card">
    <h2>
      <span>Live Touchpad Preview</span>
      <span style="font-size: 11px; color: var(--accent);" id="coordLabel">X: 0.00 | Y: 0.00</span>
    </h2>

    <div class="touchpad-box" id="padBox">
      <div class="zone zone-left" id="zoneLeft" style="width: 22%;">
        <span>LEFT EDGE</span>
        <span style="opacity:0.7;">BRIGHTNESS</span>
      </div>
      <div class="zone-center" id="zoneCenter">
        NORMAL PASS-THROUGH
      </div>
      <div class="zone zone-right" id="zoneRight" style="width: 22%;">
        <span>RIGHT EDGE</span>
        <span style="opacity:0.7;">VOLUME</span>
      </div>

      <div class="finger-dot" id="f1"></div>
      <div class="finger-dot" id="f2"></div>
    </div>

    <div class="test-bench">
      <div class="meter meter-bright">
        <div class="meter-label">Brightness Test Gauge (<span id="brightVal">50%</span>)</div>
        <div class="meter-bar"><div class="meter-fill" id="brightBar" style="width: 50%;"></div></div>
      </div>
      <div class="meter meter-vol">
        <div class="meter-label">Volume Test Gauge (<span id="volVal">50%</span>)</div>
        <div class="meter-bar"><div class="meter-fill" id="volBar" style="width: 50%;"></div></div>
      </div>
    </div>
  </div>

  <!-- 2. Gesture Editor -->
  <div class="card">
    <h2>Gesture Strip Editor</h2>
    <div class="controls-group">
      <div class="control-item">
        <div class="control-label">
          <span>Left Edge Strip Width</span>
          <span id="leftEdgeDisp">22%</span>
        </div>
        <input type="range" id="leftEdgeInput" min="5" max="35" value="22">
      </div>

      <div class="control-item">
        <div class="control-label">
          <span>Right Edge Strip Width</span>
          <span id="rightEdgeDisp">22%</span>
        </div>
        <input type="range" id="rightEdgeInput" min="5" max="35" value="22">
      </div>

      <div class="control-item">
        <div class="control-label">
          <span>Activation Deadband (Trigger Threshold)</span>
          <span id="triggerDisp">35 px</span>
        </div>
        <input type="range" id="triggerInput" min="10" max="80" value="35">
      </div>

      <div class="btn-group" style="margin-top: 10px;">
        <button class="primary" onclick="saveConfig()">Apply to Engine</button>
        <button onclick="resetDefaults()">Reset Defaults</button>
      </div>
    </div>
  </div>

  <!-- 3. Sensitivity & Timing -->
  <div class="card">
    <h2>Sensitivity & Contact Settling</h2>
    <div class="controls-group">
      <div class="control-item">
        <div class="control-label">
          <span>Volume Response Speed</span>
          <span id="volStepDisp">0.02</span>
        </div>
        <input type="range" id="volStepInput" min="0.005" max="0.05" step="0.005" value="0.02">
      </div>

      <div class="control-item">
        <div class="control-label">
          <span>Brightness Response Speed</span>
          <span id="brightStepDisp">0.02</span>
        </div>
        <input type="range" id="brightStepInput" min="0.005" max="0.05" step="0.005" value="0.02">
      </div>

      <div class="control-item">
        <div class="control-label">
          <span>Contact Settle Window</span>
          <span id="settleDisp">25 ms</span>
        </div>
        <input type="range" id="settleInput" min="10" max="60" value="25">
      </div>
    </div>
  </div>

  <!-- 4. Action Mapping Matrix -->
  <div class="card">
    <h2>Action Mapping</h2>
    <table class="table">
      <thead>
        <tr><th>Gesture</th><th>Trigger Type</th><th>Action Target</th></tr>
      </thead>
      <tbody>
        <tr>
          <td>Two-Finger Left Edge</td>
          <td>Continuous Slide</td>
          <td>
            <select id="actLeftEdge">
              <option value="brightness" selected>Display Brightness</option>
              <option value="volume">Audio Volume</option>
            </select>
          </td>
        </tr>
        <tr>
          <td>Two-Finger Right Edge</td>
          <td>Continuous Slide</td>
          <td>
            <select id="actRightEdge">
              <option value="volume" selected>Audio Volume</option>
              <option value="brightness">Display Brightness</option>
            </select>
          </td>
        </tr>
        <tr>
          <td>Three-Finger Tap</td>
          <td>Discrete Tap</td>
          <td>
            <select id="actTap3">
              <option value="key:BTN_MIDDLE" selected>Middle Mouse Click</option>
              <option value="key:KEY_PLAYPAUSE">Play / Pause</option>
            </select>
          </td>
        </tr>
        <tr>
          <td>Four-Finger Swipe Up</td>
          <td>Directional Swipe</td>
          <td>
            <select id="actSwipe4Up">
              <option value="shortcut:Overview" selected>KWin Overview / Window Grid</option>
              <option value="command:true">None</option>
            </select>
          </td>
        </tr>
      </tbody>
    </table>
  </div>

  <!-- 5. Device Profiles & Service Management -->
  <div class="card" style="grid-column: span 2;">
    <h2>Device Profiles & Service Management</h2>
    <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 16px;">
      <div style="display: flex; gap: 10px; align-items: center;">
        <span style="font-size: 13px; color: var(--muted);">Active Profile:</span>
        <select id="profileSelect" onchange="switchProfile(this.value)">
          <option value="default">Default Laptop Touchpad</option>
          <option value="ultrasensitive">Ultra-Responsive (Gaming)</option>
          <option value="wide">Wide Edges (30%)</option>
        </select>
        <button onclick="saveProfilePrompt()">Save Current as Profile</button>
      </div>

      <div class="btn-group">
        <button onclick="manageService('restart')">Restart Daemon</button>
        <button onclick="manageService('stop')" class="danger">Stop Daemon</button>
        <button onclick="manageService('start')" class="primary">Start Daemon</button>
      </div>
    </div>
  </div>

</div>

<script>
let testVol = 50;
let testBright = 50;

function updateDisplay() {
  const l = document.getElementById('leftEdgeInput').value;
  const r = document.getElementById('rightEdgeInput').value;
  document.getElementById('leftEdgeDisp').innerText = l + '%';
  document.getElementById('rightEdgeDisp').innerText = r + '%';
  document.getElementById('zoneLeft').style.width = l + '%';
  document.getElementById('zoneRight').style.width = r + '%';

  document.getElementById('triggerDisp').innerText = document.getElementById('triggerInput').value + ' px';
  document.getElementById('volStepDisp').innerText = document.getElementById('volStepInput').value;
  document.getElementById('brightStepDisp').innerText = document.getElementById('brightStepInput').value;
  document.getElementById('settleDisp').innerText = document.getElementById('settleInput').value + ' ms';
}

document.querySelectorAll('input[type="range"]').forEach(i => i.addEventListener('input', updateDisplay));

// Interactive simulation on test pad
const pad = document.getElementById('padBox');
const f1 = document.getElementById('f1');
let isTouching = false;
let startY = 0;

pad.addEventListener('mousedown', (e) => {
  isTouching = true;
  f1.style.display = 'block';
  handleMove(e);
});
window.addEventListener('mouseup', () => {
  isTouching = false;
  f1.style.display = 'none';
});
pad.addEventListener('mousemove', (e) => {
  if (isTouching) handleMove(e);
});

function handleMove(e) {
  const rect = pad.getBoundingClientRect();
  const x = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
  const y = Math.max(0, Math.min(1, (e.clientY - rect.top) / rect.height));

  f1.style.left = (x * 100) + '%';
  f1.style.top = (y * 100) + '%';
  document.getElementById('coordLabel').innerText = `X: ${x.toFixed(2)} | Y: ${y.toFixed(2)}`;

  const lRatio = document.getElementById('leftEdgeInput').value / 100.0;
  const rRatio = 1.0 - (document.getElementById('rightEdgeInput').value / 100.0);

  if (x <= lRatio) {
    testBright = Math.max(0, Math.min(100, Math.round((1.0 - y) * 100)));
    document.getElementById('brightBar').style.width = testBright + '%';
    document.getElementById('brightVal').innerText = testBright + '%';
  } else if (x >= rRatio) {
    testVol = Math.max(0, Math.min(100, Math.round((1.0 - y) * 100)));
    document.getElementById('volBar').style.width = testVol + '%';
    document.getElementById('volVal').innerText = testVol + '%';
  }
}

async function loadConfig() {
  try {
    const res = await fetch('/api/config', {
      headers: { 'X-Session-Token': window.SESSION_TOKEN || '' }
    });
    if (res.ok) {
      const cfg = await res.json();
      if (cfg.left_edge_ratio) document.getElementById('leftEdgeInput').value = Math.round(cfg.left_edge_ratio * 100);
      if (cfg.right_edge_ratio) document.getElementById('rightEdgeInput').value = Math.round(cfg.right_edge_ratio * 100);
      if (cfg.edge_trigger_threshold) document.getElementById('triggerInput').value = cfg.edge_trigger_threshold;
      if (cfg.volume_step) document.getElementById('volStepInput').value = cfg.volume_step;
      if (cfg.brightness_step) document.getElementById('brightStepInput').value = cfg.brightness_step;
      if (cfg.settle_window_ms) document.getElementById('settleInput').value = cfg.settle_window_ms;

      if (cfg.shortcuts) {
        if (cfg.shortcuts['three_finger_tap'] && document.getElementById('actTap3')) {
          document.getElementById('actTap3').value = cfg.shortcuts['three_finger_tap'];
        }
        if (cfg.shortcuts['swipe4_up'] && document.getElementById('actSwipe4Up')) {
          document.getElementById('actSwipe4Up').value = cfg.shortcuts['swipe4_up'];
        }
      }
      updateDisplay();
    }
  } catch (err) {
    console.error('Failed loading initial config', err);
  }
}

async function checkStatus() {
  try {
    const res = await fetch('/api/status', {
      headers: { 'X-Session-Token': window.SESSION_TOKEN || '' }
    });
    if (res.ok) {
      const st = await res.json();
      const dot = document.getElementById('statusDot');
      const text = document.getElementById('statusText');
      if (st.running) {
        if (st.paused) {
          dot.style.background = '#eab308';
          text.innerText = 'Daemon Paused (Grab Released)';
        } else {
          dot.style.background = '#10b981';
          text.innerText = 'Active & Grabbed (' + (st.device || 'Touchpad') + ')';
        }
      } else {
        dot.style.background = '#ef4444';
        text.innerText = 'Daemon Offline';
      }
    }
  } catch (err) {
    const dot = document.getElementById('statusDot');
    const text = document.getElementById('statusText');
    if (dot) dot.style.background = '#ef4444';
    if (text) text.innerText = 'Daemon Offline';
  }
}

async function saveConfig() {
  const cfg = {
    left_edge_ratio: document.getElementById('leftEdgeInput').value / 100.0,
    right_edge_ratio: document.getElementById('rightEdgeInput').value / 100.0,
    edge_trigger_threshold: parseFloat(document.getElementById('triggerInput').value),
    volume_step: parseFloat(document.getElementById('volStepInput').value),
    brightness_step: parseFloat(document.getElementById('brightStepInput').value),
    settle_window_ms: parseInt(document.getElementById('settleInput').value),
    shortcuts: {}
  };

  const tap3 = document.getElementById('actTap3');
  if (tap3) cfg.shortcuts['three_finger_tap'] = tap3.value;
  const s4u = document.getElementById('actSwipe4Up');
  if (s4u) cfg.shortcuts['swipe4_up'] = s4u.value;

  try {
    const res = await fetch('/api/config', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-Session-Token': window.SESSION_TOKEN || ''
      },
      body: JSON.stringify(cfg)
    });
    if (res.ok) {
      alert('Settings saved and live-reloaded into active daemon.');
      checkStatus();
    } else {
      alert('Failed to save settings: server returned ' + res.status);
    }
  } catch (err) {
    alert('Network error while saving settings.');
  }
}

async function manageService(action) {
  try {
    const res = await fetch('/api/service/' + action, {
      method: 'POST',
      headers: { 'X-Session-Token': window.SESSION_TOKEN || '' }
    });
    const data = await res.json();
    alert('Action "' + action + '": ' + (data.status || 'Dispatched'));
    setTimeout(checkStatus, 500);
  } catch (err) {
    alert('Failed to dispatch action: ' + action);
  }
}

function resetDefaults() {
  document.getElementById('leftEdgeInput').value = 22;
  document.getElementById('rightEdgeInput').value = 22;
  document.getElementById('triggerInput').value = 35;
  document.getElementById('volStepInput').value = 0.02;
  document.getElementById('brightStepInput').value = 0.02;
  document.getElementById('settleInput').value = 25;
  updateDisplay();
}

function switchProfile(name) {
  if (name === 'wide') {
    document.getElementById('leftEdgeInput').value = 30;
    document.getElementById('rightEdgeInput').value = 30;
  } else if (name === 'ultrasensitive') {
    document.getElementById('volStepInput').value = 0.04;
    document.getElementById('brightStepInput').value = 0.04;
    document.getElementById('settleInput').value = 15;
    document.getElementById('triggerInput').value = 15;
  } else {
    resetDefaults();
  }
  updateDisplay();
}

function saveProfilePrompt() {
  const name = prompt('Enter custom profile name:');
  if (name) {
    const sel = document.getElementById('profileSelect');
    const opt = document.createElement('option');
    opt.value = name.toLowerCase().replace(/[^a-z0-9]/g, '_');
    opt.innerText = name;
    opt.selected = true;
    sel.appendChild(opt);
    alert('Profile "' + name + '" saved locally to profile selector.');
  }
}

// Initial load
loadConfig();
checkStatus();
setInterval(checkStatus, 3000);
</script>

</body>
</html>
"#;
