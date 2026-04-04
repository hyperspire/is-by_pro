const domain = 'is-by.pro';

document.addEventListener('DOMContentLoaded', (event) => {
  attachEventListeners();
});

function attachEventListeners() {
  const listeners = [
    attachCopyLinkEventListener,
    attachDeletePostEventListener,
    attachNewPostEventListener,
    attachSelectUserEventListener,
    attachSelectPostEventListener,
    attachProHomeEventListener,
    attachEditPostEventListener,
    attachShowEditProEventListener,
    attachEditProEventListener,
  ];

  listeners.forEach((setup) => {
    try {
      setup();
    } catch (error) {
      console.error('Listener setup failed:', setup.name, error);
    }
  });
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
