import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

const dropArea = document.getElementById('drop-area');
const deviceList = document.querySelector('.device-list');
const progressBar = document.querySelector('.progress-bar');
const progressContainer = document.querySelector('.transfer-progress');
const fileNameDisplay = document.querySelector('.file-name');
const etaDisplay = document.querySelector('.transfer-eta');

let selectedIp = null;

progressBar.style.width = '0%';
progressContainer.style.opacity = '0';

listen('devices_updated', (event) => {
  const devices = event.payload;
  deviceList.innerHTML = '';

  if (devices.length > 0 && !selectedIp) {
    selectedIp = devices[0].ip;
  }

  devices.forEach(device => {
    const li = document.createElement('li');
    li.className = 'device-item';
    if (selectedIp === device.ip) {
      li.style.border = '1px solid #00F0FF';
      li.style.boxShadow = '0 0 10px rgba(0, 240, 255, 0.4)';
    }

    li.onclick = () => {
      selectedIp = device.ip;
      document.querySelectorAll('.device-item').forEach(el => {
        el.style.border = 'none';
        el.style.boxShadow = 'none';
      });
      li.style.border = '1px solid #00F0FF';
      li.style.boxShadow = '0 0 10px rgba(0, 240, 255, 0.4)';
    };

    li.innerHTML = `
      <div class="indicator ${device.status.toLowerCase()}"></div>
      <div class="device-info">
        <span class="device-name">${device.name}</span>
        <span class="device-status">${device.status} (${device.ip})</span>
      </div>
    `;
    deviceList.appendChild(li);
  });
});

listen('tauri://drag-drop', (event) => {
  dropArea.classList.remove('drag-over');

  if (!selectedIp) {
    alert('Please wait for a device to be discovered and connected!');
    return;
  }

  const paths = event.payload.paths;
  if (paths && paths.length > 0) {
    let filePath = paths[0];

    fileNameDisplay.textContent = filePath.split('\\').pop() || filePath;
    progressBar.style.width = '0%';
    progressContainer.style.opacity = '1';
    etaDisplay.textContent = 'Transferring...';

    invoke('transfer_file', { targetIp: selectedIp, filePath: filePath })
      .catch(err => {
        alert('Transfer Failed: ' + err);
      });
  }
});

listen('tauri://drag-enter', () => dropArea.classList.add('drag-over'));
listen('tauri://drag-leave', () => dropArea.classList.remove('drag-over'));

listen('transfer_progress', (event) => {
  let percent = event.payload;
  progressBar.style.width = percent + '%';
});

listen('transfer_complete', () => {
  progressBar.style.width = '100%';
  etaDisplay.textContent = 'Done! Saved to Downloads.';
  setTimeout(() => { progressContainer.style.opacity = '0'; }, 5000);
});

['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
  dropArea.addEventListener(eventName, e => {
    e.preventDefault();
    e.stopPropagation();
  }, false);
});
['dragenter', 'dragover'].forEach(eventName => {
  dropArea.addEventListener(eventName, () => dropArea.classList.add('drag-over'));
});
['dragleave', 'drop'].forEach(eventName => {
  dropArea.addEventListener(eventName, () => dropArea.classList.remove('drag-over'));
});
