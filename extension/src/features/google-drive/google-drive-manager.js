// ============================================================================
// Google Drive Integration Manager
// Manages Google Drive authentication and file operations
// ============================================================================

import { showStatusMessage } from '../../sidebar/management/stateManagement.js';

// ============================================================================
// Modal Management
// ============================================================================

let isModalOpen = false;

/**
 * Initialize Google Drive UI
 */
export function initializeGoogleDrive() {
  const settingsBtn = document.getElementById('googleSettingsBtn');
  const modal = document.getElementById('googleSettingsModal');
  const closeBtn = document.getElementById('closeGoogleSettingsBtn');
  const saveCredsBtn = document.getElementById('saveGoogleCredsBtn');
  const loginBtn = document.getElementById('loginGoogleBtn');
  const logoutBtn = document.getElementById('logoutGoogleBtn');

  if (!settingsBtn || !modal) {
    console.warn('Google Drive UI elements not found');
    return;
  }

  // Open modal
  settingsBtn.addEventListener('click', async () => {
    await openGoogleSettingsModal();
  });

  // Close modal
  closeBtn.addEventListener('click', () => {
    closeGoogleSettingsModal();
  });

  // Close on outside click
  modal.addEventListener('click', (e) => {
    if (e.target === modal) {
      closeGoogleSettingsModal();
    }
  });

  // Save credentials
  saveCredsBtn.addEventListener('click', async () => {
    await saveGoogleCredentials();
  });

  // Login
  loginBtn.addEventListener('click', async () => {
    await startGoogleLogin();
  });

  // Logout
  logoutBtn.addEventListener('click', async () => {
    await logoutFromGoogle();
  });

  console.log('✅ Google Drive manager initialized');
}

/**
 * Open Google settings modal
 */
async function openGoogleSettingsModal() {
  const modal = document.getElementById('googleSettingsModal');
  modal.style.display = 'flex';
  isModalOpen = true;

  // Update UI state
  await updateModalState();
}

/**
 * Close Google settings modal
 */
function closeGoogleSettingsModal() {
  const modal = document.getElementById('googleSettingsModal');
  modal.style.display = 'none';
  isModalOpen = false;
}

/**
 * Update modal UI based on current state
 */
async function updateModalState() {
  try {
    // Check if logged in
    const isLoggedIn = await invokeCommand('is_google_logged_in');

    // Check if credentials are set
    const hasCredentials = await invokeCommand('has_google_credentials');

    const credentialsForm = document.getElementById('googleCredentialsForm');
    const loginSection = document.getElementById('googleLoginSection');
    const loggedInSection = document.getElementById('googleLoggedInSection');
    const statusDiv = document.getElementById('googleLoginStatus');

    // Hide all sections initially
    credentialsForm.style.display = 'none';
    loginSection.style.display = 'none';
    loggedInSection.style.display = 'none';
    statusDiv.style.display = 'none';

    if (isLoggedIn) {
      // User is logged in
      loggedInSection.style.display = 'block';
      statusDiv.style.display = 'block';
      statusDiv.style.backgroundColor = 'rgba(34, 197, 94, 0.1)';
      statusDiv.style.borderLeft = '3px solid #22c55e';
      statusDiv.textContent = '✓ Connected to Google Drive';
    } else if (hasCredentials) {
      // Credentials set but not logged in
      loginSection.style.display = 'block';
      statusDiv.style.display = 'block';
      statusDiv.style.backgroundColor = 'rgba(59, 130, 246, 0.1)';
      statusDiv.style.borderLeft = '3px solid #3b82f6';
      statusDiv.textContent = 'ℹ Credentials configured. Please login to continue.';
    } else {
      // No credentials set
      credentialsForm.style.display = 'block';
      statusDiv.style.display = 'block';
      statusDiv.style.backgroundColor = 'rgba(251, 191, 36, 0.1)';
      statusDiv.style.borderLeft = '3px solid #fbbf24';
      statusDiv.textContent = '⚠ Please configure Google OAuth credentials first';
    }
  } catch (error) {
    console.error('Failed to update modal state:', error);
    showStatusMessage('Failed to check Google Drive status', 'error');
  }
}

/**
 * Save Google credentials
 */
async function saveGoogleCredentials() {
  const clientId = document.getElementById('googleClientId').value.trim();
  const clientSecret = document.getElementById('googleClientSecret').value.trim();

  if (!clientId || !clientSecret) {
    showStatusMessage('Please enter both Client ID and Client Secret', 'error');
    return;
  }

  try {
    await invokeCommand('set_google_credentials', { clientId, clientSecret });
    showStatusMessage('Google credentials saved successfully', 'success');

    // Clear inputs for security
    document.getElementById('googleClientId').value = '';
    document.getElementById('googleClientSecret').value = '';

    // Update UI
    await updateModalState();
  } catch (error) {
    console.error('Failed to save credentials:', error);
    showStatusMessage(`Failed to save credentials: ${error}`, 'error');
  }
}

/**
 * Start Google login flow
 */
async function startGoogleLogin() {
  try {
    showStatusMessage('Opening browser for Google authentication...', 'info');

    const authUrl = await invokeCommand('start_google_login');
    console.log('Google auth URL:', authUrl);

    // Wait a bit for the OAuth flow to complete
    // The backend will handle the callback automatically
    setTimeout(async () => {
      await checkLoginStatus();
    }, 5000); // Check after 5 seconds

    showStatusMessage('Please complete authentication in your browser', 'info');
  } catch (error) {
    console.error('Failed to start Google login:', error);
    showStatusMessage(`Login failed: ${error}`, 'error');
  }
}

/**
 * Check login status periodically
 */
async function checkLoginStatus() {
  if (!isModalOpen) return;

  try {
    const isLoggedIn = await invokeCommand('is_google_logged_in');

    if (isLoggedIn) {
      showStatusMessage('Successfully logged in to Google Drive!', 'success');
      await updateModalState();
    } else {
      // Keep checking if modal is still open
      setTimeout(() => checkLoginStatus(), 2000);
    }
  } catch (error) {
    console.error('Failed to check login status:', error);
  }
}

/**
 * Logout from Google
 */
async function logoutFromGoogle() {
  try {
    await invokeCommand('google_logout');
    showStatusMessage('Logged out from Google Drive', 'success');
    await updateModalState();
  } catch (error) {
    console.error('Failed to logout:', error);
    showStatusMessage(`Logout failed: ${error}`, 'error');
  }
}

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Invoke Tauri command
 * Works for both Tauri desktop app and Chrome extension
 */
async function invokeCommand(command, args = {}) {
  // Check if running in Tauri
  if (window.__TAURI__) {
    const { invoke } = window.__TAURI__.core;
    return await invoke(command, args);
  }

  // Running in Chrome extension - communicate with background script
  return new Promise((resolve, reject) => {
    chrome.runtime.sendMessage(
      {
        action: 'tauri_command',
        command: command,
        args: args,
      },
      (response) => {
        if (chrome.runtime.lastError) {
          reject(chrome.runtime.lastError.message);
        } else if (response && response.error) {
          reject(response.error);
        } else {
          resolve(response);
        }
      }
    );
  });
}

/**
 * Get access token (for future API calls)
 */
export async function getGoogleAccessToken() {
  try {
    return await invokeCommand('get_google_access_token');
  } catch (error) {
    console.error('Failed to get access token:', error);
    throw error;
  }
}

/**
 * Check if user is logged in
 */
export async function isGoogleLoggedIn() {
  try {
    return await invokeCommand('is_google_logged_in');
  } catch (error) {
    console.error('Failed to check login status:', error);
    return false;
  }
}
