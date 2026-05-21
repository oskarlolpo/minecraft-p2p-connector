# Supabase Auth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a Supabase Authentication overlay that blocks the main application until the user logs in or registers.

**Architecture:** We will initialize the Supabase client, add an auth modal overlay to the DOM, and update `main.js` to manage the authentication state and hide the overlay when a valid session exists.

**Tech Stack:** Vanilla JavaScript, Vite, Tailwind CSS, Supabase JS.

---

### Task 1: Setup Supabase Client

**Files:**
- Modify: `package.json`
- Create: `src/supabase.js`

- [ ] **Step 1: Install Supabase JS client**
Run: `npm install @supabase/supabase-js`

- [ ] **Step 2: Create the Supabase client file**
Create `src/supabase.js` with the connection details.
```javascript
import { createClient } from '@supabase/supabase-js';

const supabaseUrl = 'https://mjbqlrzcijxiontrbhak.supabase.co';
const supabaseKey = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Im1qYnFscnpjaWp4bG9udHJiaGFrIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzkxOTA0MTcsImV4cCI6MjA5NDc2NjQxN30.exvRp1J7iEgs7qXTePe1Mi9dcQfUli8PIOdlteWJa6M';

export const supabase = createClient(supabaseUrl, supabaseKey);
```

- [ ] **Step 3: Commit**
```bash
git add package.json package-lock.json src/supabase.js
git commit -m "feat: setup supabase js client"
```

---

### Task 2: Add Auth Overlay UI to HTML

**Files:**
- Modify: `src/index.html`

- [ ] **Step 1: Add the Auth Overlay immediately inside the body**
Open `src/index.html` and add this right after `<body>`:
```html
  <div id="auth-overlay" class="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-90 backdrop-blur-md">
    <div class="bg-[var(--bg-secondary)] border border-[var(--border-color)] p-8 rounded-xl shadow-2xl max-w-md w-full mx-4">
      <div class="text-center mb-6">
        <h2 class="text-2xl font-bold mb-2">Welcome</h2>
        <p class="text-[var(--text-secondary)] text-sm">Sign in or register to continue</p>
      </div>

      <div id="auth-error-message" class="hidden mb-4 p-3 bg-red-500 bg-opacity-20 border border-red-500 rounded text-red-400 text-sm">
      </div>

      <form id="auth-form" class="space-y-4">
        <div>
          <label class="block text-sm font-medium mb-1">Email</label>
          <input type="email" id="auth-email" class="w-full bg-[var(--bg-tertiary)] border border-[var(--border-color)] rounded p-2 focus:border-[var(--accent)] outline-none transition-colors" required>
        </div>
        <div>
          <label class="block text-sm font-medium mb-1">Password</label>
          <input type="password" id="auth-password" class="w-full bg-[var(--bg-tertiary)] border border-[var(--border-color)] rounded p-2 focus:border-[var(--accent)] outline-none transition-colors" required minlength="6">
        </div>
        
        <div class="flex gap-3 pt-2">
          <button type="submit" id="btn-login" class="flex-1 primary-button py-2">Sign In</button>
          <button type="button" id="btn-register" class="flex-1 ghost-button border border-[var(--border-color)] py-2">Register</button>
        </div>
      </form>
    </div>
  </div>
```

- [ ] **Step 2: Commit**
```bash
git add src/index.html
git commit -m "feat: add auth overlay ui"
```

---

### Task 3: Hook up Auth Logic in main.js

**Files:**
- Modify: `src/main.js`

- [ ] **Step 1: Import Supabase and add DOM references**
At the top of `src/main.js`, add the import and DOM elements:
```javascript
import { supabase } from "./supabase.js";

const authOverlayEl = document.querySelector("#auth-overlay");
const authFormEl = document.querySelector("#auth-form");
const authEmailEl = document.querySelector("#auth-email");
const authPasswordEl = document.querySelector("#auth-password");
const btnRegisterEl = document.querySelector("#btn-register");
const authErrorEl = document.querySelector("#auth-error-message");
```

- [ ] **Step 2: Add Auth initialization and listeners**
Add this function to manage the overlay and form logic:
```javascript
function showError(msg) {
  if (authErrorEl) {
    authErrorEl.textContent = msg;
    authErrorEl.classList.remove("hidden");
  }
}

async function initAuth() {
  // Check current session
  const { data: { session } } = await supabase.auth.getSession();
  if (session) {
    authOverlayEl?.classList.add("hidden");
  }

  // Listen for changes
  supabase.auth.onAuthStateChange((event, session) => {
    if (session) {
      authOverlayEl?.classList.add("hidden");
    } else {
      authOverlayEl?.classList.remove("hidden");
    }
  });

  // Handle Login
  authFormEl?.addEventListener("submit", async (e) => {
    e.preventDefault();
    authErrorEl?.classList.add("hidden");
    const email = authEmailEl.value;
    const password = authPasswordEl.value;
    
    const { error } = await supabase.auth.signInWithPassword({ email, password });
    if (error) showError(error.message);
  });

  // Handle Register
  btnRegisterEl?.addEventListener("click", async () => {
    authErrorEl?.classList.add("hidden");
    const email = authEmailEl.value;
    const password = authPasswordEl.value;
    if (!email || !password || password.length < 6) {
      showError("Please enter email and password (min 6 chars)");
      return;
    }
    
    const { error } = await supabase.auth.signUp({ email, password });
    if (error) {
      showError(error.message);
    } else {
      showError("Registration successful! Check your email for confirmation or sign in directly if email confirmation is disabled.");
      authErrorEl.classList.replace("bg-red-500", "bg-green-500");
      authErrorEl.classList.replace("border-red-500", "border-green-500");
      authErrorEl.classList.replace("text-red-400", "text-green-400");
    }
  });
}

// Call initAuth when app starts
initAuth();
```

- [ ] **Step 3: Run the dev server to test manually**
Run `npm run dev` to verify the login screen shows up and works.
Expected: The screen overlays the app, and clicking "Register" creates a user in Supabase.

- [ ] **Step 4: Commit**
```bash
git add src/main.js
git commit -m "feat: implement supabase auth logic"
```
