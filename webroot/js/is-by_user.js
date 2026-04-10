const domain = 'is-by.pro';

document.addEventListener('DOMContentLoaded', (event) => {
  attachEventListeners();
});

function attachEventListeners() {
  stopDMPolling();

  const listeners = [
    attachAcknowledgePostEventListener,
    attachCopyLinkEventListener,
    attachDeletePostEventListener,
    attachNewPostEventListener,
    attachSelectUserEventListener,
    attachSelectPostEventListener,
    attachProHomeEventListener,
    attachEditPostEventListener,
    attachShowEditProEventListener,
    attachEditProEventListener,
    attachDirectMessageEventListeners,
  ];

  listeners.forEach((setup) => {
    try {
      setup();
    } catch (error) {
      console.error('Listener setup failed:', setup.name, error);
    }
  });
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
    link.addEventListener('click', (event) => {
      const href = link.getAttribute('href') || '';
      if (href === '' || href === 'javascript:void(0);') {
        event.preventDefault();
        dmPanel.style.display = dmPanel.style.display === 'none' ? 'block' : 'none';
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

  dmButtons.forEach((button) => {
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

  dmForm.addEventListener('submit', async (event) => {
    event.preventDefault();

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

async function updateUnreadDMCount() {
  const unreadCountNode = document.getElementById('dm-unread-count');
  const ibUID = getCurrentIBUID();

  if (!unreadCountNode || Number.isNaN(ibUID)) {
    return;
  }

  try {
    const response = await fetch(`https://${domain}/v1/dm/unreadcount`, {
      method: 'GET',
      headers: {
        'Accept': 'application/json',
        'ib-uid': String(ibUID),
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
    const response = await fetch(`https://${domain}/v1/dm/messages?${params.toString()}`, {
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
      return `
        <div class="${senderClass}">
          <p class="dm-message-meta"><strong>${escapeHTML(message.sender_user)}</strong> <span>${escapeHTML(message.timestamp)}</span></p>
          <p class="dm-message-body">${escapeHTML(message.message)}</p>
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
    const post = ibPostForm.querySelector('input[name="post"]').value;

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
  const textFieldPost = document.querySelector('input[name="post"]');
  const charCountDiv = document.getElementById(`${counter}`);

  if (!textFieldPost || !charCountDiv) {
    return;
  }

  textFieldPost.addEventListener('input', (event) => {
    const charCountPost = event.target.value.length;
    charCountDiv.textContent = charCountPost + '/1024';
    if (charCountPost > 1000) {
      charCountDiv.style.color = 'red';
    } else {
      charCountDiv.style.color = 'green';
    }
  });

  textFieldPost.addEventListener('focus', () => {
    charCountDiv.textContent = '0/1024';
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

  fetch(`https://${domain}/v1/profile/${ibUser}`, {
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
