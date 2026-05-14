// ============================================================================
// Google Drive Integration UI
// Handles Google Drive authentication and file operations in desktop app
// ============================================================================

// Use invoke from main.js (already declared globally)
// Using window.invoke directly (declared in main.js)

let isCheckingStatus = false;

/**
 * Initialize Google Drive UI
 */
function initGoogleDriveUI() {
  console.log('Initializing Google Drive UI...');

  const saveCredsBtn = document.getElementById('save-google-creds-btn');
  const loginBtn = document.getElementById('google-login-btn');
  const logoutBtn = document.getElementById('google-logout-btn');
  const testBtn = document.getElementById('test-google-connection-btn');

  if (saveCredsBtn) {
    saveCredsBtn.addEventListener('click', handleSaveCredentials);
  }

  if (loginBtn) {
    loginBtn.addEventListener('click', handleLogin);
  }

  if (logoutBtn) {
    logoutBtn.addEventListener('click', handleLogout);
  }

  if (testBtn) {
    testBtn.addEventListener('click', handleTestConnection);
  }

  // Check initial status - defer to ensure Tauri is ready
  setTimeout(() => {
    updateUI();
  }, 100);
}

/**
 * Update UI based on current state
 */
async function updateUI() {
  try {
    const isLoggedIn = await window.invoke('is_google_logged_in');
    const hasCredentials = await window.invoke('has_google_credentials');

    const loginSection = document.getElementById('google-login-section');
    const connectedSection = document.getElementById('google-connected-section');
    const statusBadge = document.getElementById('google-status-badge');

    // Hide all sections initially
    loginSection.style.display = 'none';
    connectedSection.style.display = 'none';

    if (isLoggedIn) {
      // User is logged in
      connectedSection.style.display = 'block';
      statusBadge.textContent = '✓ Connected';
      statusBadge.style.backgroundColor = '#22c55e';
      statusBadge.style.color = 'white';
      statusBadge.style.padding = '4px 12px';
      statusBadge.style.borderRadius = '12px';
      statusBadge.style.fontSize = '12px';
      statusBadge.style.fontWeight = '500';
    } else if (hasCredentials) {
      // Credentials set but not logged in
      loginSection.style.display = 'block';
      statusBadge.textContent = '⚠ Not Logged In';
      statusBadge.style.backgroundColor = '#fbbf24';
      statusBadge.style.color = 'white';
      statusBadge.style.padding = '4px 12px';
      statusBadge.style.borderRadius = '12px';
      statusBadge.style.fontSize = '12px';
      statusBadge.style.fontWeight = '500';
    } else {
      // No credentials
      statusBadge.textContent = 'Not Configured';
      statusBadge.style.backgroundColor = '#9ca3af';
      statusBadge.style.color = 'white';
      statusBadge.style.padding = '4px 12px';
      statusBadge.style.borderRadius = '12px';
      statusBadge.style.fontSize = '12px';
      statusBadge.style.fontWeight = '500';
    }
  } catch (error) {
    console.error('Failed to update UI:', error);
    showToast('Failed to check Google Drive status', 'error');
  }
}

/**
 * Save Google credentials
 */
async function handleSaveCredentials() {
  const clientId = document.getElementById('google-client-id').value.trim();
  const clientSecret = document.getElementById('google-client-secret').value.trim();

  if (!clientId || !clientSecret) {
    showToast('Please enter both Client ID and Client Secret', 'error');
    return;
  }

  try {
    await window.invoke('set_google_credentials', { clientId, clientSecret });
    showToast('Credentials saved successfully!', 'success');

    // Clear inputs for security
    document.getElementById('google-client-id').value = '';
    document.getElementById('google-client-secret').value = '';

    // Update UI
    await updateUI();
  } catch (error) {
    console.error('Failed to save credentials:', error);
    showToast(`Failed to save credentials: ${error}`, 'error');
  }
}

/**
 * Start Google login flow
 */
async function handleLogin() {
  try {
    showToast('Opening browser for authentication...', 'info');

    await window.invoke('start_google_login');

    showToast('Please complete authentication in your browser', 'info');

    // Start checking login status
    startLoginStatusCheck();
  } catch (error) {
    console.error('Failed to start login:', error);
    showToast(`Login failed: ${error}`, 'error');
  }
}

/**
 * Check login status periodically
 */
function startLoginStatusCheck() {
  if (isCheckingStatus) return;

  isCheckingStatus = true;
  console.log('[GoogleDrive] Starting login status polling loop...');
  showToast('Waiting for authentication...', 'info');

  const checkStatus = async () => {
    if (!isCheckingStatus) return;

    try {
      console.log('[GoogleDrive] Checking status...');
      const isLoggedIn = await window.invoke('is_google_logged_in');

      if (isLoggedIn) {
        console.log('[GoogleDrive] ✅ Login detected! Updating UI.');
        showToast('Successfully logged in to Google Drive!', 'success');
        isCheckingStatus = false;
        await updateUI();
      } else {
        // Continue checking every 1.5 seconds
        setTimeout(checkStatus, 1500);
      }
    } catch (error) {
      console.error('[GoogleDrive] Failed to check login status:', error);
      // Don't stop on error, network might be flaky or backend busy
      setTimeout(checkStatus, 2000);
    }
  };

  // Start immediately
  checkStatus();

  // Stop checking after 5 minutes
  setTimeout(() => {
    if (isCheckingStatus) {
      isCheckingStatus = false;
      console.log('[GoogleDrive] Stopped checking login status (timeout)');
      showToast('Login timed out. Please try again.', 'warning');
    }
  }, 300000);
}

/**
 * Logout from Google
 */
async function handleLogout() {
  try {
    await window.invoke('google_logout');
    showToast('Logged out from Google Drive', 'success');
    await updateUI();
  } catch (error) {
    console.error('Failed to logout:', error);
    showToast(`Logout failed: ${error}`, 'error');
  }
}

/**
 * Test Google connection
 */
async function handleTestConnection() {
  try {
    showToast('Testing connection & Refreshing UI...', 'info');

    // 1. Sprawdź token
    const accessToken = await window.invoke('get_google_access_token');

    // 2. Wymuś odświeżenie UI
    console.log('[GoogleDrive] Manually refreshing UI status...');
    await updateUI();

    if (accessToken) {
      showToast('Connection confirmed! UI Updated.', 'success');
    } else {
      showToast('No access token found.', 'warning');
    }
  } catch (error) {
    console.error('Connection test failed:', error);
    // Jeśli błąd zawiera "Command ... not found", to znaczy że backend nie został przebudowany
    if (error.toString().includes("not found")) {
        showToast('CRITICAL: Backend commands missing. Please rebuild app.', 'error');
    } else {
        showToast(`Error: ${error}`, 'error');
    }
    await updateUI(); // Próbuj odświeżyć mimo błędu
  }
}

/**
 * Show toast notification
 */
function showToast(message, type = 'info') {
  const toast = document.getElementById('toast');
  if (!toast) return;

  toast.textContent = message;
  toast.className = `toast ${type}`;
  toast.style.display = 'block';

  setTimeout(() => {
    toast.style.display = 'none';
  }, 3000);
}

// Export to window for main.js
window.initGoogleDriveUI = initGoogleDriveUI;