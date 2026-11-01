import vue from "@vitejs/plugin-vue";

// The page reaches the game server through its own origin; in development
// Vite passes those requests on (VITE_GAME_PORT / VITE_POW_PORT change where).
const gamePort = process.env.VITE_GAME_PORT || 6767;
const powPort = process.env.VITE_POW_PORT || 3001;

// https://vite.dev/config/
export default {
  plugins: [vue()],
  server: {
    proxy: {
      "/challenge": `http://127.0.0.1:${powPort}`,
      "/ws": { target: `ws://127.0.0.1:${gamePort}`, ws: true },
    },
  },
};
