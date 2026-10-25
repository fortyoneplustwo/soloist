const SPOTIFY_GREEN = "#1db954";
const DEVICE_ID = "qtpi";

// state
const curTrack = {
  uri: "",
};

// subscribe event listeners
const playlists = document.getElementsByTagName("article");
for (const playlist of playlists) {
  playlist.addEventListener("click", async (event) => {
    event.stopPropagation();
    const uri = event.currentTarget.dataset.id;
    console.log("clicked target:", event.currentTarget);
    await fetch(`/play-playlist?id=${uri}`);
  });
}

const connectBtn = document.getElementById("connect-btn");
console.log(connectBtn);
connectBtn.addEventListener("click", async () => {
  console.log("connect button ");
  await fetch("/activate");
});

const playBtn = document.getElementById("play-btn");
playBtn.addEventListener("click", async (event) => {
  console.log("play/pause button clicked", event.target);
  if (event.target.innerText == "play") {
    await fetch(`/play?id=${curTrack.uri}`);
  } else {
    await fetch(`/pause?id=${curTrack.uri}`);
  }
});

// subscribe to playback events via SSE
const eventSource = new EventSource("/subscribe");

const cover = document.getElementById("cover");
const track = document.getElementById("title");
const player = document.getElementById("player");

eventSource.onmessage = (event) => {
  const state = JSON.parse(event.data);
  console.log("message:", state);

  switch (state.type) {
    case "playback_state": {
      if (state.is_active) {
        connectBtn.hidden = true;
        playBtn.hidden = false;
        player.style.backgroundColor = SPOTIFY_GREEN;
      } else {
        connectBtn.hidden = false;
        playBtn.hidden = true;
        player.style.backgroundColor = "";
      }

      curTrack.uri = state.item.uri.split(":").at(-1);
      cover.src = state.item.decorations.visual_identity.cover[0].url;
      track.textContent = state.item.decorations.identity.name;
      playBtn.innerText = state.status == "paused" ? "play" : "pause";
      break;
    }
    case "playback_changed": {
      playBtn.innerText = state.status == "playing" ? "pause" : "play";
      break;
    }
    case "device_changed": {
      if (state.device_name == DEVICE_ID && state.is_active) {
        connectBtn.hidden = true;
        playBtn.hidden = false;
        player.style.backgroundColor = SPOTIFY_GREEN;
      } else {
        connectBtn.hidden = false;
        playBtn.hidden = true;
        player.style.backgroundColor = "";
      }
      break;
    }
  }
};

eventSource.addEventListener("open", () => {
  console.log("new connection established");
});

eventSource.addEventListener("error", () => {
  console.log("error establishing connection");
});
