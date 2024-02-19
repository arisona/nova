const contentsId = 'nova-contents';
const rangeIds = ['nova-red', 'nova-green', 'nova-blue', 'nova-brightness', 'nova-speed'];
const resetId = 'nova-reset';
const reloadId = 'nova-reload';
const selectedContent = 'nova-content';

document.addEventListener('DOMContentLoaded', function () {
    // fetch state from server and set ui elements
    const xhr = new XMLHttpRequest();
    xhr.open('GET', '/api/get-state');
    xhr.send();
    xhr.onload = function () {
        if (xhr.status != 200) {
            console.warn(`get-state: ${xhr.status}: ${xhr.statusText}`);
            return;
        }
        try {
            const response = JSON.parse(xhr.response);
            rangeIds.forEach(id => document.getElementById(id).value = response[id]);
            const contents = document.getElementById(contentsId);
            for (let i = 0; i < response[contentsId].length; ++i) {
                contents.appendChild(new Option(response[contentsId][i], i, false, i == response[selectedContent]));
            }
        } catch (e) {
            console.error(e);
        }
    };

    // add event listeners to ui elements
    document.getElementById(contentsId).addEventListener('change', event => {
        call(event.target.id, event.target.value);
    });

    rangeIds.forEach(id => document.getElementById(id).addEventListener('input', event => {
        call(event.target.id, event.target.value);
    }));

    document.getElementById(resetId).addEventListener('click', event => {
        call(event.target.id, null);
    });

    document.getElementById(reloadId).addEventListener('click', event => {
        call(event.target.id, null);
    });
});

function call(id, value) {
    const xhr = new XMLHttpRequest();
    if (value)
        xhr.open('GET', `/api/${id}?value=${value}`);
    else
        xhr.open('GET', `/api/${id}`);
    xhr.send();
}
