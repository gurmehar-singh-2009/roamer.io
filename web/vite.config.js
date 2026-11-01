import vue from "@vitejs/plugin-vue";

// The page reaches the game server at /ws on its own origin; in development
// Vite passes that on (VITE_GAME_PORT changes where).
const gamePort = process.env.VITE_GAME_PORT || 6767;

// https://vite.dev/config/
export default {
  plugins: [vue()],
  server: {
    proxy: {
      "/ws": { target: `ws://127.0.0.1:${gamePort}`, ws: true },
    },
  },
};
