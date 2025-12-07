// Check sync status ONCE on page load
(function () {
  const serverDate = new Date(SERVER_TIME);
  const clientDate = new Date();
  const diffSeconds = Math.abs((clientDate - serverDate) / 1000);

  const statusEl = document.getElementById("sync-status");
  if (diffSeconds < 60) {
    statusEl.textContent = "✓ Synced";
    statusEl.style.color = "#4ade80";
  } else {
    statusEl.textContent =
      "⏳ Not synced (off by " + Math.round(diffSeconds) + "s)";
    statusEl.style.color = "#f87171";
  }
})();

async function syncTime() {
  const btn = document.querySelector("button");
  const status = document.getElementById("status");

  btn.disabled = true;
  btn.textContent = "Syncing...";

  try {
    const timestamp = Math.floor(Date.now() / 1000);
    const response = await fetch("/set_time?timestamp=" + timestamp);
    const data = await response.json();

    if (data.ok) {
      status.textContent = "✓ Time synced successfully!";
      status.style.color = "#4ade80";
      setTimeout(() => location.reload(), 1000);
    } else {
      throw new Error("Sync failed");
    }
  } catch (err) {
    status.textContent = "✗ Failed to sync: " + err.message;
    status.style.color = "#f87171";
  }

  btn.disabled = false;
  btn.textContent = "Sync Time";
}

// Helper: Convert seconds from midnight to HH:MM format
function formatTime(secondsFromMidnight) {
  const hours = Math.floor(secondsFromMidnight / 3600);
  const minutes = Math.floor((secondsFromMidnight % 3600) / 60);
  return `${hours.toString().padStart(2, "0")}:${minutes
    .toString()
    .padStart(2, "0")}`;
}

// Helper: Format duration as "Xh Ym" or "Xm"
function formatDuration(seconds) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

// Helper: Convert HH:MM to seconds from midnight
function timeToSeconds(timeStr) {
  const [hours, minutes] = timeStr.split(":").map(Number);
  return hours * 3600 + minutes * 60;
}

// Helper: Convert minutes to seconds
function minutesToSeconds(minutes) {
  return minutes * 60;
}

// Fetch and display aspersor info
async function loadInfo() {
  try {
    const response = await fetch("/get_info");
    const data = await response.json();

    const container = document.getElementById("aspersores");
    container.innerHTML = data.aspersores
      .map((a) => {
        const startTime = formatTime(a.init_time);
        const endTime = formatTime(a.init_time + a.duration);

        return `
        <div class="aspersor ${a.on ? "on" : "off"}">
          <div class="aspersor-header">
            <span class="name">${a.name}</span>
            <span class="status">${a.on ? "🟢 ON" : "⚫ OFF"}</span>
          </div>
          <div class="schedule">
            🕐 ${startTime} → ${endTime} (${formatDuration(a.duration)})
          </div>
          <div class="edit-row">
            <label>Start: <input type="time" id="start-${
              a.name
            }" value="${startTime}"></label>
            <label>End: <input type="time" id="end-${
              a.name
            }" value="${endTime}"></label>
            <button class="save-btn" onclick="updateAspersor('${
              a.name
            }')">💾 Save</button>
          </div>
          <button onclick="toggleAspersor('${a.name}')">${
          a.on ? "Turn Off" : "Turn On"
        }</button>
        </div>
      `;
      })
      .join("");

    const modeBtn = document.getElementById("manual-mode");
    modeBtn.className = data.manual_mode ? "mode-btn manual" : "mode-btn auto";
    modeBtn.innerHTML = data.manual_mode
      ? "🔧 Manual Mode <small>(click for Auto)</small>"
      : "⏰ Auto Mode <small>(click for Manual)</small>";
  } catch (err) {
    console.error("Failed to load info:", err);
  }
}

// Add toggle function for manual mode
async function toggleManualMode() {
  await fetch("/toggle/manual_mode");
  loadInfo(); // Refresh to show new state
}

async function toggleAspersor(name) {
  await fetch("/toggle/" + name);
  loadInfo(); // Refresh
}

// Update aspersor schedule
async function updateAspersor(name) {
  const startInput = document.getElementById(`start-${name}`);
  const endInput = document.getElementById(`end-${name}`);

  const initTime = timeToSeconds(startInput.value);
  const endTime = timeToSeconds(endInput.value);

  // Calculate duration (handle midnight crossing)
  let duration;
  if (endTime > initTime) {
    duration = endTime - initTime;
  } else {
    // End time is next day (e.g., 23:00 → 01:00)
    duration = 24 * 3600 - initTime + endTime;
  }

  // Validate
  if (duration <= 0 || duration > 24 * 3600) {
    alert("Invalid time range");
    return;
  }

  try {
    const response = await fetch(
      `/update_aspersor/${name}?init_time=${initTime}&duration=${duration}`
    );
    const data = await response.json();

    if (data.ok) {
      loadInfo(); // Refresh to show updated schedule
    }
  } catch (err) {
    console.error("Failed to update:", err);
  }
}

// Load info on page load
loadInfo();

// Live digital clock based on server time
(function () {
  const serverDate = new Date(SERVER_TIME);
  const clientNow = new Date();
  const drift = serverDate.getTime() - clientNow.getTime();

  function updateClock() {
    const now = new Date(Date.now() + drift);

    const hours = now.getHours().toString().padStart(2, "0");
    const minutes = now.getMinutes().toString().padStart(2, "0");
    const seconds = now.getSeconds().toString().padStart(2, "0");

    const day = now.getDate().toString().padStart(2, "0");
    const month = (now.getMonth() + 1).toString().padStart(2, "0");
    const year = now.getFullYear();

    // Update time
    document.getElementById("clock-hours").textContent = hours;
    document.getElementById("clock-minutes").textContent = minutes;
    document.getElementById("clock-seconds").textContent = seconds;

    // Update date DD/MM/YYYY
    document.getElementById(
      "clock-date"
    ).textContent = `${day}/${month}/${year}`;

    // Blink the colon
    const colons = document.querySelectorAll(".clock-colon");
    colons.forEach((c) => (c.style.opacity = seconds % 2 === 0 ? "1" : "0.2"));
  }

  updateClock();
  setInterval(updateClock, 1000);

  // Sync status
  const diffSeconds = Math.abs(drift / 1000);
  const statusEl = document.getElementById("sync-status");
  if (diffSeconds < 60) {
    statusEl.innerHTML = '<span class="synced">● SYNCED</span>';
  } else {
    statusEl.innerHTML = `<span class="not-synced">● OFF ${Math.round(
      diffSeconds
    )}s</span>`;
  }
})();
