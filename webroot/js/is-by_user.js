
document.addEventListener('DOMContentLoaded', (event) => {
  attachEventListeners();
});

function attachEventListeners() {
  stopDMPolling();

  const listeners = [
    attachUsernameHoverCardEventListener,
    attachAcknowledgePostEventListener,
    attachCopyLinkEventListener,
    attachPinPostEventListener,
    attachDeletePostEventListener,
    attachNewPostEventListener,
    attachSelectUserEventListener,
    attachSelectPostEventListener,
    attachProHomeEventListener,
    attachEditPostEventListener,
    attachShowEditProEventListener,
    attachEditProEventListener,
    attachDirectMessageEventListeners,
    attachPostsInfiniteScrollEventListener,
    attachFollowersInfiniteScrollEventListener,
    attachDMContactsInfiniteScrollEventListener,
    attachSSEListener,
  ];

  listeners.forEach((setup) => {
    try {
      setup();
    } catch (error) {
      console.error('Listener setup failed:', setup.name, error);
    }
  });
}

function attachUsernameHoverCardEventListener() {
  const profileLinks = Array.from(document.querySelectorAll('a[href*="/v1/profile/"]'));
  if (profileLinks.length === 0) {
    return;
  }

  let hoverCard = document.getElementById('user-hover-card');
  if (!hoverCard) {
    hoverCard = document.createElement('div');
    hoverCard.id = 'user-hover-card';
    hoverCard.style.display = 'none';
    document.body.appendChild(hoverCard);
  }

  let showTimer = null;
  let hideTimer = null;

  const clearTimers = () => {
    if (showTimer) {
      window.clearTimeout(showTimer);
      showTimer = null;
    }
    if (hideTimer) {
      window.clearTimeout(hideTimer);
      hideTimer = null;
    }
  };

  const hideCard = () => {
    hoverCard.style.display = 'none';
  };

  const scheduleHide = () => {
    if (hideTimer) {
      window.clearTimeout(hideTimer);
    }
    hideTimer = window.setTimeout(() => {
      hideCard();
    }, 180);
  };

  hoverCard.addEventListener('mouseenter', () => {
    if (hideTimer) {
      window.clearTimeout(hideTimer);
      hideTimer = null;
    }
  });

  hoverCard.addEventListener('mouseleave', () => {
    scheduleHide();
  });

  profileLinks.forEach((link) => {
    if (link.dataset.hoverCardBound === '1') {
      return;
    }

    const username = extractProfileUsernameFromHref(link.getAttribute('href'));
    if (!username) {
      return;
    }

    link.dataset.hoverCardBound = '1';
    link.dataset.hoverUsername = username;

    const showForLink = () => {
      clearTimers();
      showTimer = window.setTimeout(async () => {
        await showUsernameHoverCard(link, username, hoverCard);
      }, 180);
    };

    link.addEventListener('mouseenter', showForLink);
    link.addEventListener('focus', showForLink);
    link.addEventListener('mouseleave', scheduleHide);
    link.addEventListener('blur', scheduleHide);
  });
}

function extractProfileUsernameFromHref(href) {
  if (!href) {
    return '';
  }

  try {
    const parsed = new URL(href, window.location.origin);
    const segments = parsed.pathname.split('/').filter(Boolean);
    if (segments.length < 3) {
      return '';
    }
    if (segments[0] !== 'v1' || segments[1] !== 'profile') {
      return '';
    }
    return decodeURIComponent(segments[2] || '').trim();
  } catch (error) {
    return '';
  }
}

function positionHoverCard(anchor, hoverCard) {
  const rect = anchor.getBoundingClientRect();
  const cardWidth = hoverCard.offsetWidth || 260;
  const cardHeight = hoverCard.offsetHeight || 140;

  let left = window.scrollX + rect.left;
  let top = window.scrollY + rect.bottom + 8;

  const maxLeft = window.scrollX + window.innerWidth - cardWidth - 10;
  if (left > maxLeft) {
    left = Math.max(window.scrollX + 10, maxLeft);
  }

  const maxTop = window.scrollY + window.innerHeight - cardHeight - 10;
  if (top > maxTop) {
    top = Math.max(window.scrollY + 10, window.scrollY + rect.top - cardHeight - 8);
  }

  hoverCard.style.left = `${left}px`;
  hoverCard.style.top = `${top}px`;
}

async function showUsernameHoverCard(anchor, username, hoverCard) {
  hoverCard.innerHTML = '<p><em>Loading...</em></p>';
  hoverCard.style.display = 'block';
  positionHoverCard(anchor, hoverCard);

  try {
    const response = await fetch(`/v1/user/hover/${encodeURIComponent(username)}`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
      },
    });

    if (!response.ok) {
      hoverCard.innerHTML = '<p><em>Unavailable</em></p>';
      positionHoverCard(anchor, hoverCard);
      return;
    }

    const data = await response.json();
    if (data.success !== true) {
      hoverCard.innerHTML = '<p><em>Unavailable</em></p>';
      positionHoverCard(anchor, hoverCard);
      return;
    }

    let actionsHTML = '';
    if (data.show_follow === true) {
      actionsHTML += `
        <form class="user-hover-action-form" action="/v1/follow" method="POST">
          <input type="hidden" name="target_user" value="${escapeHTML(data.username)}">
          <input class="user-hover-action user-hover-follow" type="submit" value="Follow">
        </form>`;
    }

    if (data.show_unfollow === true) {
      actionsHTML += `
        <form class="user-hover-action-form" action="/v1/unfollow" method="POST">
          <input type="hidden" name="target_user" value="${escapeHTML(data.username)}">
          <input class="user-hover-action user-hover-unfollow" type="submit" value="Unfollow">
        </form>`;
    }

    if (actionsHTML === '' && data.logged_in !== true) {
      actionsHTML = '<p class="user-hover-muted"><em>Login to follow users.</em></p>';
    }

    const rankIcon = data.rank_icon ? escapeHTML(data.rank_icon) : '/images/ranks/pvt.svg';

    hoverCard.innerHTML = `
      <p class="user-hover-title"><img class="user-hover-rank-icon" src="${rankIcon}" alt="Rank" width="14" height="14">@${escapeHTML(data.username)}</p>
      <p class="user-hover-rank">Rank ${escapeHTML(data.rank_level)}: ${escapeHTML(data.rank_name)}</p>
      <p class="user-hover-acks">Unique ACKs: ${escapeHTML(data.unique_acknowledgments)}</p>
      <div class="user-hover-actions">${actionsHTML}</div>`;
    positionHoverCard(anchor, hoverCard);
  } catch (error) {
    hoverCard.innerHTML = '<p><em>Unavailable</em></p>';
    positionHoverCard(anchor, hoverCard);
  }
}

function getCurrentIBUID() {
  const selectUserForm = document.getElementById('select-user-form');
  const ibUIDField = selectUserForm?.querySelector('input[name="ib_uid"]');
  return ibUIDField ? Number(ibUIDField.value) : NaN;
}

function stopDMPolling() {
  if (window.dmUnreadInterval) {
    window.clearInterval(window.dmUnreadInterval);
    window.dmUnreadInterval = null;
  }

  if (window.dmThreadInterval) {
    window.clearInterval(window.dmThreadInterval);
    window.dmThreadInterval = null;
  }
}

function attachDirectMessageEventListeners() {
  const dmPanel = document.getElementById('dm-panel');
  const dmForm = document.getElementById('dm-form');
  const dmThread = document.getElementById('dm-thread');
  const dmStatus = document.getElementById('dm-message-status');
  const dmTargetLabel = document.getElementById('dm-target-user');
  const dmTargetInput = document.getElementById('dm-target-user-input');
  const dmUnreadCount = document.getElementById('dm-unread-count');
  const dmInboxLinks = document.querySelectorAll('.dm-inbox-display');
  const dmButtons = document.querySelectorAll('.open-dm');

  if (dmUnreadCount) {
    updateUnreadDMCount();
    window.dmUnreadInterval = window.setInterval(updateUnreadDMCount, 5000);
  }

  if (!dmPanel || !dmForm || !dmThread || !dmTargetLabel || !dmTargetInput) {
    return;
  }

  dmInboxLinks.forEach((link) => {
    if (link.dataset.dmInboxBound === '1') {
      return;
    }
    link.dataset.dmInboxBound = '1';

    link.addEventListener('click', (event) => {
      const href = link.getAttribute('href') || '';
      if (href === '' || href === 'javascript:void(0);') {
        event.preventDefault();
        dmPanel.style.display = dmPanel.style.display === 'none' ? 'block' : 'none';
        initPushNotifications();
      }
    });
  });

  if (dmTargetInput.value.trim() !== '') {
    const targetUser = dmTargetInput.value.trim();
    dmPanel.style.display = 'block';
    dmTargetLabel.textContent = targetUser;
    loadDMThread(targetUser);
    if (window.dmThreadInterval) {
      window.clearInterval(window.dmThreadInterval);
    }
    window.dmThreadInterval = window.setInterval(() => loadDMThread(targetUser), 3000);
  }

  attachDMOpenButtons(dmButtons, dmPanel, dmTargetLabel, dmTargetInput);

  if (dmForm.dataset.dmSubmitBound === '1') {
    return;
  }
  dmForm.dataset.dmSubmitBound = '1';

  dmForm.addEventListener('submit', async (event) => {
    event.preventDefault();
    initPushNotifications();

    const targetUser = dmTargetInput.value.trim();
    const messageField = document.getElementById('dm-message-input');
    const message = messageField?.value.trim() || '';
    const ibUID = getCurrentIBUID();

    if (!targetUser || !message || Number.isNaN(ibUID)) {
      setDMStatus(dmStatus, false, 'Target user and message are required');
      return;
    }

    try {
      const response = await fetch(dmForm.action, {
        method: dmForm.method,
        headers: {
          'Content-Type': 'application/json',
          'Accept': 'application/json',
          'ib-uid': String(ibUID),
        },
        body: JSON.stringify({ target_user: targetUser, message: message }),
      });

      const data = await response.json();
      if (data.success === true) {
        messageField.value = '';
        setDMStatus(dmStatus, true, data.message || 'Message sent');
        await loadDMThread(targetUser);
        await updateUnreadDMCount();
      } else {
        setDMStatus(dmStatus, false, data.message || 'Failed to send message');
      }
    } catch (error) {
      setDMStatus(dmStatus, false, String(error));
      console.error('dm-send-error:', error);
    }
  });
}

function attachDMOpenButtons(buttons, dmPanel, dmTargetLabel, dmTargetInput) {
  buttons.forEach((button) => {
    if (button.dataset.dmButtonBound === '1') {
      return;
    }
    button.dataset.dmButtonBound = '1';

    button.addEventListener('click', (event) => {
      event.preventDefault();
      const targetUser = button.dataset.targetUser;
      if (!targetUser) {
        return;
      }

      dmPanel.style.display = 'block';
      dmTargetLabel.textContent = targetUser;
      dmTargetInput.value = targetUser;
      loadDMThread(targetUser);
      if (window.dmThreadInterval) {
        window.clearInterval(window.dmThreadInterval);
      }
      window.dmThreadInterval = window.setInterval(() => loadDMThread(targetUser), 3000);
    });
  });
}

async function updateUnreadDMCount() {
  const unreadCountNode = document.getElementById('dm-unread-count');

  if (!unreadCountNode) {
    return;
  }

  try {
    const response = await fetch(`/v1/dm/unreadcount`, {
      method: 'GET',
      cache: 'no-store',
      headers: {
        'Accept': 'application/json',
      },
    });

    const data = await response.json();
    if (data.success === true) {
      unreadCountNode.textContent = String(data.unread_count);
      unreadCountNode.style.color = data.unread_count > 0 ? '#ff3300' : '#33cc44';
    }
  } catch (error) {
    console.error('dm-unread-error:', error);
  }
}

async function loadDMThread(targetUser) {
  const dmThread = document.getElementById('dm-thread');
  const ibUID = getCurrentIBUID();

  if (!dmThread || !targetUser || Number.isNaN(ibUID)) {
    return;
  }

  try {
    const params = new URLSearchParams({ target_user: targetUser });
    const response = await fetch(`/v1/dm/messages?${params.toString()}`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
        'ib-uid': String(ibUID),
      },
    });

    const data = await response.json();
    if (data.success !== true) {
      return;
    }

    if (!Array.isArray(data.messages) || data.messages.length === 0) {
      dmThread.innerHTML = '<p><em>:[[ :no-direct-messages: ]]:</em></p>';
      await updateUnreadDMCount();
      return;
    }

    dmThread.innerHTML = data.messages.map((message) => {
      const senderClass = message.is_mine ? 'dm-message dm-message-mine' : 'dm-message dm-message-theirs';
      // First escape the entire message
      let processedMessage = escapeHTML(message.message);
      // Then replace escaped marker patterns with actual links
      processedMessage = processedMessage.replace(/\|\|\|LINK\|\|\|([^|]*)\|\|\|([^|]*)\|\|\|/g,
        (match, url, text) => `<a href="${url}" target="_blank">${text}</a>`
      );
      return `
        <div class="${senderClass}">
          <p class="dm-message-meta"><strong>${escapeHTML(message.sender_user)}</strong> <span>${escapeHTML(message.timestamp)}</span></p>
          <p class="dm-message-body">${processedMessage}</p>
        </div>`;
    }).join('');

    dmThread.scrollTop = dmThread.scrollHeight;
    await updateUnreadDMCount();
  } catch (error) {
    console.error('dm-thread-error:', error);
  }
}

function escapeHTML(value) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function setDMStatus(node, success, message) {
  if (!node) {
    return;
  }

  const cssClass = success ? 'success' : 'failure';
  node.innerHTML = `<em class="${cssClass}">${escapeHTML(message)}</em>`;
}

function urlBase64ToUint8Array(base64String) {
  const padding = '='.repeat((4 - base64String.length % 4) % 4);
  const base64 = (base64String + padding)
    .replace(/\-/g, '+')
    .replace(/_/g, '/');

  const rawData = window.atob(base64);
  const outputArray = new Uint8Array(rawData.length);

  for (let i = 0; i < rawData.length; ++i) {
    outputArray[i] = rawData.charCodeAt(i);
  }
  return outputArray;
}

async function initPushNotifications() {
  try {
    if (!('serviceWorker' in navigator) || !('PushManager' in window)) {
      return;
    }

    if (Notification.permission === 'denied') {
      return;
    }

    const permission = await Notification.requestPermission();
    if (permission !== 'granted') {
      console.log('Push notification permission denied.');
      return;
    }

    const registration = await navigator.serviceWorker.ready;
    if (!registration) {
      return;
    }

    // Force an update to ensure the latest sw.js is active
    try {
      await registration.update();
    } catch (e) {
      console.error('Failed to update service worker:', e);
    }

    const keyResponse = await fetch('/v1/push/public-key');
    if (!keyResponse.ok) {
      console.error('Failed to fetch VAPID public key');
      return;
    }
    const vapidPublicKey = await keyResponse.text();
    const applicationServerKey = urlBase64ToUint8Array(vapidPublicKey);

    const subscription = await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: applicationServerKey
    });

    const subJson = subscription.toJSON();
    const payload = {
      endpoint: subJson.endpoint,
      keys: {
        p256dh: subJson.keys.p256dh,
        auth: subJson.keys.auth
      }
    };

    const subscribeResponse = await fetch('/v1/push/subscribe', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(payload)
    });

    if (subscribeResponse.ok) {
      console.log('Push subscription saved successfully.');
    } else {
      console.error('Failed to save push subscription to backend.');
    }
  } catch (error) {
    console.error('Failed to initialize push notifications:', error);
  }
}

function attachCopyLinkEventListener() {
  const copyLinks = document.querySelectorAll('.copy-link');

  copyLinks.forEach((link) => {
    link.addEventListener('click', async (event) => {
      event.preventDefault();

      try {
        await navigator.clipboard.writeText(window.location.href);
      } catch (error) {
        console.log('Failed to copy link: ', error);
      }
    });
  });
}

function getLastPathSegment(urlString) {
  const segments = new URL(urlString).pathname.split('/').filter(Boolean);
  return segments.length > 0 ? segments[segments.length - 1] : '';
}

function attachSelectUserEventListener() {
  const selectUserLinks = document.querySelectorAll('.select-user');
  const selectUserForm = document.getElementById('select-user-form');
  const ibUID = selectUserForm.querySelector('input[name="ibuid"]').value;
  const ibAuthToken = selectUserForm.querySelector('input[name="ibauthtoken"]').value;

  selectUserLinks.forEach((link) => {
    link.addEventListener('click', async (event) => {
      event.preventDefault();
      const path = getLastPathSegment(link.href);

      const headers = {
        'Content-Type': 'application/json',
        'Accept': 'text/html',
        'ib-uid': ibUID,
        'ib-authtoken': ibAuthToken,
        'ib-selecteduser': path
      };

      try {
        const params = new URLSearchParams({
          'ibuid': ibUID,
          'ibauthtoken': ibAuthToken,
          'ibselecteduser': path
        });

        const response = await fetch(`${selectUserForm.action}?${params.toString()}`, {
          method: 'GET',
          headers: headers
        });

        const data = await response.text();
        generateIBFormSuccess(data);
      } catch (error) {
        console.log('select-user-message:', error);
        console.error('select-user-message:', error);
      }
    });
  });
}

function attachNewPostEventListener() {
  const links = document.querySelectorAll('a.post-form-display');
  const postFormSection = document.getElementById('post-form-section');
  const ibPostForm = document.querySelector('#post-form');
  const cancelButton = document.getElementById('post-cancel');

  if (!postFormSection || !ibPostForm || !cancelButton) {
    return;
  }

  characterCounter('post-character-count');

  for (let link of links) {
    link.addEventListener('click', (event) => {
      event.preventDefault();
      postFormSection.style.display = 'block'; // display the form section
      window.scrollTo({ top: 0, behavior: 'smooth' });
    });
  }

  cancelButton.addEventListener('click', (event) => {
    postFormSection.style.display = 'none'; // hide the form section
  });

  ibPostForm.addEventListener('submit', (event) => {
    event.preventDefault();

    let ibUsername = '';
    const ibUID = Number(ibPostForm.querySelector('input[name="ib_uid"]').value);
    const ibUser = ibPostForm.querySelector('input[name="ib_user"]').value;
    const post = ibPostForm.querySelector('[name="post"]').value;

    if (Number.isNaN(ibUID)) {
      generateIBFormMessageFailure('post-message', 'Invalid user id');
      return;
    }

    fetch(ibPostForm.action, {
      method: ibPostForm.method,
      headers: {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'ib-uid': ibUID
      },
      body: JSON.stringify({ 'ib_uid': ibUID, 'ib_user': ibUser, 'post': post }),
    })
      .then(async response => {
        ibUsername = response.headers.get('ib_user');
        return response.json();
      })
      .then(data => {
        if (data.success === true) {
          generateIBFormMessageSuccess('post-message', data.message);
          generateIBPostFormSuccess(ibUsername, ibUID);
        } else if (data.success === false) {
          generateIBFormMessageFailure('post-message', data.message);
        }
      })
      .catch(error => generateIBFormMessageFailure('post-message', error))
  });

}

function attachAcknowledgePostEventListener() {
  const acknowledgeLinks = document.querySelectorAll('.ack-post');

  acknowledgeLinks.forEach((link) => {
    link.addEventListener('click', (event) => {
      event.preventDefault();

      const acknowledgeForm = link.closest('.post')?.querySelector('.ack-post-form');
      if (acknowledgeForm) {
        acknowledgeForm.requestSubmit();
      }
    });
  });
}

function attachSelectPostEventListener() {
  const selectPostLinks = document.querySelectorAll('.show-post');

  selectPostLinks.forEach((link) => {
    link.addEventListener('click', (event) => {
      event.preventDefault();

      const showPostForm = link.closest('.post')?.querySelector('.show-post-form');
      if (showPostForm) {
        showPostForm.requestSubmit();
      }
    });
  });
}

function attachProHomeEventListener() {
  const selectUserLinks = document.querySelectorAll('.pro-home-display');
  const selectUserForm = document.getElementById('select-user-form');
  const ibUID = selectUserForm.querySelector('input[name="ibuid"]').value;
  const ibAuthToken = selectUserForm.querySelector('input[name="ibauthtoken"]').value;

  selectUserLinks.forEach((link) => {
    link.addEventListener('click', async (event) => {
      event.preventDefault();
      const path = getLastPathSegment(link.href);

      const headers = {
        'Content-Type': 'application/json',
        'Accept': 'text/html',
        'ib-uid': ibUID,
        'ib-authtoken': ibAuthToken,
        'ib-selecteduser': path
      };

      try {
        const params = new URLSearchParams({
          'ibuid': ibUID,
          'ibauthtoken': ibAuthToken,
          'ibselecteduser': path
        });

        const response = await fetch(`${selectUserForm.action}?${params.toString()}`, {
          method: 'GET',
          headers: headers
        });

        const data = await response.text();
        generateIBFormSuccess(data);

      } catch (error) {
        generateIBFormMessageFailure('edit-pro-message', error);
        console.error('Error:', error);
      }
    });
  });
}

function attachEditPostEventListener() {
  const editPostLinks = document.querySelectorAll('.edit-post');

  editPostLinks.forEach((link) => {
    link.addEventListener('click', (event) => {
      event.preventDefault();

      const editPostForm = link.closest('.post')?.querySelector('.edit-post-form');
      if (editPostForm) {
        editPostForm.requestSubmit();
      }
    });
  });
}

function attachDeletePostEventListener() {
  const deletePostLinks = document.querySelectorAll('.delete-post');

  deletePostLinks.forEach((link) => {
    link.addEventListener('click', (event) => {
      event.preventDefault();

      const deletePostForm = link.closest('.post')?.querySelector('.delete-post-form');

      if (deletePostForm) {
        deletePostForm.requestSubmit();
      }
    });
  });
}

function attachShowEditProEventListener() {
  const showEditProLinks = document.querySelectorAll('.show-edit-profile');
  const editProForm = document.getElementById('edit-profile-form');

  if (!editProForm) {
    return;
  }

  showEditProLinks.forEach((link) => {
    link.addEventListener('click', (event) => {
      event.preventDefault();
      editProForm.requestSubmit();
    });
  });
}

function attachEditProEventListener() {
  const ibEditProForm = document.querySelector('#editpro');

  ibEditProForm.addEventListener('submit', (event) => {
    event.preventDefault();

    const ibUID = ibEditProForm.querySelector('input[name="ibuid"]').value;
    const ibAuthToken = ibEditProForm.querySelector('input[name="ibauthtoken"]').value;
    const ibIBP = ibEditProForm.querySelector('input[name="ibibp"]').value;
    const ibLocation = ibEditProForm.querySelector('input[name="iblocation"]').value;
    const ibServices = ibEditProForm.querySelector('input[name="ibservices"]').value;
    const ibWebsite = ibEditProForm.querySelector('input[name="ibwebsite"]').value;
    const ibGitHub = ibEditProForm.querySelector('input[name="ibgithub"]').value;

    fetch(ibEditProForm.action, {
      method: ibEditProForm.method,
      headers: {
        'Content-Type': 'application/json',
        'Accept': 'text/html',
        'ib-uid': ibUID,
        'ib-authtoken': ibAuthToken
      },
      body: JSON.stringify({ 'ibuid': ibUID, 'ibauthtoken': ibAuthToken, 'ibibp': ibIBP, 'iblocation': ibLocation, 'ibservices': ibServices, 'ibwebsite': ibWebsite, 'ibgithub': ibGitHub })
    })
      .then(async response => {
        for (let [key, value] of response.headers.entries()) {
          console.log(`${key}: ${value}`);
        }

        await response.text();
      })
      .then(data => generateIBFormSuccess(data))
      .catch(error => generateIBFormMessageFailure('edit-pro-message', error))
  });
}

function characterCounter(counter) {
  const textFieldPost = document.querySelector('[name="post"]');
  const charCountDiv = document.getElementById(`${counter}`);

  if (!textFieldPost || !charCountDiv) {
    return;
  }

  textFieldPost.addEventListener('input', (event) => {
    const charCountPost = event.target.value.length;
    charCountDiv.textContent = charCountPost + '/4096';
    if (charCountPost > 1000) {
      charCountDiv.style.color = 'red';
    } else {
      charCountDiv.style.color = 'green';
    }
  });

  textFieldPost.addEventListener('focus', () => {
    charCountDiv.textContent = '0/4096';
  });
}

function generateIBFormMessageSuccess(id, content) {
  document.getElementById(`${id}`).innerHTML = `
  <em class="success">${content}</em>`;
}

function generateIBFormMessageFailure(id, content) {
  document.getElementById(`${id}`).innerHTML = `
  <em class="failure">${content}</em>`;
}

function generateIBPostFormSuccess(ibUser, ibUID) {
  const headers = new Headers();
  headers.append('ib-uid', ibUID);

  fetch(`/v1/profile/${ibUser}`, {
    method: 'GET',
    headers: headers
  })
    .then(response => response.text())
    .then(data => {
      generateIBFormSuccess(data);
    })
    .catch(error => {
      console.error('Error:', error);
    });
  attachEventListeners();
}

function generateIBFormSuccess(content) {
  if (content !== undefined) {
    document.documentElement.innerHTML = content;
  }
  attachEventListeners();
}

function attachPostsInfiniteScrollEventListener() {
  const section = document.getElementById('selected-user-posts-section');
  const sentinel = document.getElementById('posts-load-sentinel');
  if (!section || !sentinel) return;

  const ibUID = section.dataset.ibUid;
  const ibUser = section.dataset.ibUser;
  if (!ibUID || !ibUser) return;

  const feedType = section.dataset.feedType || 'profile';

  let loading = false;

  const observer = new IntersectionObserver(async (entries) => {
    if (!entries[0].isIntersecting || loading) return;
    loading = true;

    try {
      let data;

      if (feedType === 'warroom') {
        const params = new URLSearchParams({
          ib_uid: ibUID,
          ib_user: ibUser,
          offset: section.dataset.warRoomOffset || '0',
          limit: '20'
        });
        const resp = await fetch(`/api/v1/warroom/posts?${params}`);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        data = await resp.json();
        if (typeof data.next_offset === 'number') {
          section.dataset.warRoomOffset = String(data.next_offset);
        }
      } else {
        const allPosts = Array.from(section.querySelectorAll('.post[data-timestamp]'));
        const lastPost = allPosts[allPosts.length - 1];
        const beforeTimestamp = lastPost ? lastPost.dataset.timestamp : '';
        const params = new URLSearchParams({ ib_uid: ibUID, ib_user: ibUser });
        if (beforeTimestamp) params.set('before_timestamp', beforeTimestamp);
        const resp = await fetch(`/api/v1/posts?${params}`);
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        data = await resp.json();
      }

      if (data.posts_html) {
        sentinel.insertAdjacentHTML('beforebegin', data.posts_html);
        attachAcknowledgePostEventListener();
        attachDeletePostEventListener();
        attachEditPostEventListener();
        attachSelectPostEventListener();
        attachUsernameHoverCardEventListener();
        attachCopyLinkEventListener();
      }
      if (!data.has_more) {
        observer.disconnect();
        sentinel.remove();
      }
    } catch (e) {
      console.error('posts-scroll-error:', e);
    }
    loading = false;
  }, { rootMargin: '300px' });

  observer.observe(sentinel);
}

function attachFollowersInfiniteScrollEventListener() {
  const section = document.getElementById('followers-section');
  const container = document.getElementById('followers-container');
  const sentinel = document.getElementById('followers-load-sentinel');
  if (!section || !container || !sentinel) return;

  const ibUID = section.dataset.ibUid;
  if (!ibUID) return;

  let loading = false;

  const observer = new IntersectionObserver(async (entries) => {
    if (!entries[0].isIntersecting || loading) return;
    loading = true;

    try {
      const params = new URLSearchParams({
        ib_uid: ibUID,
        offset: section.dataset.followersOffset || '0',
        limit: '20'
      });

      const resp = await fetch(`/api/v1/followers?${params}`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data = await resp.json();

      if (data.followers_html) {
        sentinel.insertAdjacentHTML('beforebegin', data.followers_html);
        const dmButtons = container.querySelectorAll('.open-dm');
        const dmPanel = document.getElementById('dm-panel');
        const dmTargetLabel = document.getElementById('dm-target-user');
        const dmTargetInput = document.getElementById('dm-target-user-input');
        if (dmPanel && dmTargetLabel && dmTargetInput) {
          attachDMOpenButtons(dmButtons, dmPanel, dmTargetLabel, dmTargetInput);
        }
        attachUsernameHoverCardEventListener();
      }

      if (typeof data.next_offset === 'number') {
        section.dataset.followersOffset = String(data.next_offset);
      }

      if (!data.has_more) {
        observer.disconnect();
        sentinel.remove();
      }
    } catch (e) {
      console.error('followers-scroll-error:', e);
    }

    loading = false;
  }, { rootMargin: '300px' });

  observer.observe(sentinel);
}

function attachDMContactsInfiniteScrollEventListener() {
  const contactList = document.getElementById('dm-contact-list');
  const sentinel = document.getElementById('dm-contacts-load-sentinel');
  if (!contactList || !sentinel) return;

  const ibUID = contactList.dataset.ibUid;
  const ibUser = contactList.dataset.ibUser;
  if (!ibUID || !ibUser) return;

  let loading = false;

  const observer = new IntersectionObserver(async (entries) => {
    if (!entries[0].isIntersecting || loading) return;
    loading = true;

    try {
      const params = new URLSearchParams({
        ib_uid: ibUID,
        ib_user: ibUser,
        offset: contactList.dataset.contactsOffset || '0',
        limit: '20'
      });

      const resp = await fetch(`/api/v1/inbox/contacts?${params}`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data = await resp.json();

      if (data.contacts_html) {
        sentinel.insertAdjacentHTML('beforebegin', data.contacts_html);

        const dmButtons = contactList.querySelectorAll('.open-dm');
        const dmPanel = document.getElementById('dm-panel');
        const dmTargetLabel = document.getElementById('dm-target-user');
        const dmTargetInput = document.getElementById('dm-target-user-input');
        if (dmPanel && dmTargetLabel && dmTargetInput) {
          attachDMOpenButtons(dmButtons, dmPanel, dmTargetLabel, dmTargetInput);
        }
      }

      if (typeof data.next_offset === 'number') {
        contactList.dataset.contactsOffset = String(data.next_offset);
      }

      if (!data.has_more) {
        observer.disconnect();
        sentinel.remove();
      }
    } catch (e) {
      console.error('dm-contacts-scroll-error:', e);
    }

    loading = false;
  }, { rootMargin: '300px' });

  observer.observe(sentinel);
}

function attachSSEListener() {
  if (window.ibSSEConnection) {
    return;
  }

  const ibUID = getCurrentIBUID();
  if (Number.isNaN(ibUID)) {
    return;
  }

  window.ibSSEConnection = new EventSource('/v1/events');

  window.ibSSEConnection.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data);
      if (data.event_type === 'dm') {
        showToast(data.message, 'toast-dm');
        updateUnreadDMCount();
      } else if (data.event_type === 'reinforcement') {
        showToast(data.message, 'toast-reinforcement');
      } else {
        showToast(data.message, '');
      }
    } catch (error) {
      console.error('Error parsing SSE event:', error);
    }
  };

  window.ibSSEConnection.onerror = (error) => {
    console.error('SSE connection error', error);
  };
}

function showToast(message, className) {
  let container = document.getElementById('toast-container');
  if (!container) {
    container = document.createElement('div');
    container.id = 'toast-container';
    document.body.appendChild(container);
  }

  const toast = document.createElement('div');
  toast.className = `toast ${className}`;
  toast.innerHTML = `<span>${escapeHTML(message)}</span>`;

  container.appendChild(toast);

  setTimeout(() => {
    toast.classList.add('toast-fade-out');
    toast.addEventListener('animationend', () => {
      toast.remove();
    });
  }, 5000);
}

function attachPinPostEventListener() {
  const pinLinks = document.querySelectorAll('.pin-post-link');

  pinLinks.forEach((link) => {
    if (link.dataset.pinBound === '1') return;
    link.dataset.pinBound = '1';

    link.addEventListener('click', async (event) => {
      event.preventDefault();

      const postDiv = link.closest('.post');
      const pid = postDiv?.getAttribute('data-postid');

      let ibUID = getCurrentIBUID();
      if (Number.isNaN(ibUID)) {
        ibUID = Number(document.querySelector('input[name="ibuid"]')?.value || document.querySelector('input[name="ib_uid"]')?.value);
      }

      const deleteForm = postDiv?.querySelector('.delete-post-form');
      let ibUser = deleteForm?.querySelector('input[name="ib_user"]')?.value;
      if (!ibUser) {
        ibUser = document.querySelector('input[name="ib_user"]')?.value || document.querySelector('input[name="ibuser"]')?.value || "";
      }

      if (!pid || Number.isNaN(ibUID)) {
        console.error("Missing pid or ibUID");
        return;
      }

      try {
        const response = await fetch('/v1/pinpost', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            'Accept': 'application/json',
            'ib-uid': String(ibUID)
          },
          body: JSON.stringify({ ib_uid: ibUID, ib_user: ibUser, pid: pid })
        });

        const data = await response.json();
        if (data.success === true) {
          const url = new URL(window.location.href);
          url.searchParams.set("t", Date.now());
          window.location.href = url.toString();
        } else {
          console.error("Failed to pin post: ", data.message);
        }
      } catch (error) {
        console.error("Error pinning post: ", error);
      }
    });
  });
}

async function renderGithubCard(card) {
  if (card.dataset.rendered) return;
  card.dataset.rendered = "true";

  const repoStr = card.getAttribute('data-repo');
  if (!repoStr) return;

  const [owner, repo] = repoStr.split('/');
  if (!owner || !repo) return;

  try {
    const response = await fetch(`/v1/github/repo?owner=${encodeURIComponent(owner)}&repo=${encodeURIComponent(repo)}`);
    if (!response.ok) {
      throw new Error("Failed to fetch repo");
    }

    const data = await response.json();

    const starIcon = `<svg aria-hidden="true" height="16" viewBox="0 0 16 16" version="1.1" width="16" data-view-component="true" style="fill: currentColor;"><path fill-rule="evenodd" d="M8 .25a.75.75 0 01.673.418l1.882 3.815 4.21.612a.75.75 0 01.416 1.279l-3.046 2.97.719 4.192a.75.75 0 01-1.088.791L8 12.347l-3.766 1.98a.75.75 0 01-1.088-.79l.72-4.194L.818 6.374a.75.75 0 01.416-1.28l4.21-.611L7.327.668A.75.75 0 018 .25zm0 2.445L6.615 5.5a.75.75 0 01-.564.41l-3.097.45 2.24 2.184a.75.75 0 01.216.664l-.528 3.084 2.769-1.456a.75.75 0 01.698 0l2.77 1.456-.53-3.084a.75.75 0 01.216-.664l2.24-2.183-3.096-.45a.75.75 0 01-.564-.41L8 2.694v.001z"></path></svg>`;
    const forkIcon = `<svg aria-hidden="true" height="16" viewBox="0 0 16 16" version="1.1" width="16" data-view-component="true" style="fill: currentColor;"><path fill-rule="evenodd" d="M5 3.25a.75.75 0 11-1.5 0 .75.75 0 011.5 0zm0 2.122a2.25 2.25 0 10-1.5 0v.878A2.25 2.25 0 005.75 8.5h1.5v2.128a2.251 2.251 0 101.5 0V8.5h1.5a2.25 2.25 0 002.25-2.25v-.878a2.25 2.25 0 10-1.5 0v.878a.75.75 0 01-.75.75h-4.5A.75.75 0 015 6.25v-.878zm3.75 7.378a.75.75 0 11-1.5 0 .75.75 0 011.5 0zm3-8.75a.75.75 0 100-1.5.75.75 0 000 1.5z"></path></svg>`;

    let languageColor = "#8b949e";
    if (data.language === "Rust") languageColor = "#dea584";
    if (data.language === "JavaScript") languageColor = "#f1e05a";
    if (data.language === "TypeScript") languageColor = "#3178c6";
    if (data.language === "Python") languageColor = "#3572A5";
    if (data.language === "Go") languageColor = "#00ADD8";

    const langDot = data.language ? `<span style="display:inline-block; width:10px; height:10px; border-radius:50%; background-color:${languageColor}; margin-right:4px;"></span>${data.language}` : '';

    card.innerHTML = `
      <div class="github-repo-card-header">
        <img src="${data.owner_avatar_url}" class="github-repo-card-avatar" alt="owner avatar">
        <a href="https://github.com/${repoStr}" target="_blank" rel="noopener" class="github-repo-card-title">${repoStr}</a>
      </div>
      <p class="github-repo-card-description">${data.description || 'No description provided.'}</p>
      <div class="github-repo-card-stats">
        ${data.language ? `<div class="github-repo-card-stat">${langDot}</div>` : ''}
        <div class="github-repo-card-stat">${starIcon} ${data.stargazers_count}</div>
        <div class="github-repo-card-stat">${forkIcon} ${data.forks_count}</div>
      </div>
    `;
  } catch (e) {
    console.error(e);
    card.innerHTML = `<a href="https://github.com/${repoStr}" target="_blank" rel="noopener" class="github-repo-card-title">${repoStr}</a>`;
  }
}

document.addEventListener("DOMContentLoaded", function () {
  document.querySelectorAll('.github-repo-card').forEach(renderGithubCard);

  const observer = new MutationObserver((mutations) => {
    mutations.forEach((mutation) => {
      mutation.addedNodes.forEach((node) => {
        if (node.nodeType === 1) {
          if (node.classList && node.classList.contains('github-repo-card')) {
            renderGithubCard(node);
          }
          node.querySelectorAll('.github-repo-card').forEach(renderGithubCard);
        }
      });
    });
  });

  observer.observe(document.body, { childList: true, subtree: true });
});
