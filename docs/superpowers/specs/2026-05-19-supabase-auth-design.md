# Supabase Auth & Profile Integration Design

## 1. Overview
The goal is to integrate Supabase into the P2P Minecraft Connector app to handle user authentication, profile management, and later, real-time presence (active hosts). We will start by implementing the Auth UI and connection logic.

## 2. Authentication Flow (Option 1)
- **Startup Gate**: When the user opens the application, we check if there is an active Supabase session.
- **Unauthenticated State**: A styled, dark-themed Auth Overlay is shown over the main interface. The user cannot interact with the main lobby until authenticated.
- **Login/Register UI**: 
  - Fields: Email, Password.
  - Buttons: "Sign In", "Sign Up", "Sign In with Google".
- **Authenticated State**: The Auth Overlay is hidden, and the user's Supabase session data is loaded into the app's `state.profile`.

## 3. Technical Implementation
- **Dependencies**: Install `@supabase/supabase-js` via `npm install @supabase/supabase-js`.
- **Initialization**: Create a `supabase.js` module or add logic in `main.js` using the provided project URL (`https://mjbqlrzcijxiontrbhak.supabase.co`) and Anon Key.
- **UI Elements**: Add the new Auth HTML structure into `index.html` using Tailwind CSS classes that match the existing OLED/Dark theme.
- **State Management**: Update `main.js` to listen for Supabase auth state changes (`supabase.auth.onAuthStateChange`).

## 4. Next Steps (Future Phases)
- After Auth is working, implement Profile completion (choosing a unique `@username` and nickname).
- Implement avatar uploads to Supabase Storage.
- Migrate the Active Hosts (Lobby) from Ably to Supabase Realtime Postgres.
