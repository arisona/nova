import './style.css'

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
            rangeIds.forEach(id => {
                const range = document.getElementById(id) as HTMLInputElement;
                range.value = response[id];
            });
            const contents = document.getElementById(contentsId) as HTMLSelectElement;
            for (let i = 0; i < response[contentsId].length; ++i) {
                contents.appendChild(new Option(response[contentsId][i], i.toString(), false, i == response[selectedContent]));
            }
        } catch (e) {
            console.error(e);
        }
    };

    // add event listeners to ui elements
    document.getElementById(contentsId)!.addEventListener('change', event => {
        const target = event.target as HTMLSelectElement;
        call(target.id, target.value);
    });

    rangeIds.forEach(id => document.getElementById(id)!.addEventListener('input', event => {
        const target = event.target as HTMLInputElement;
        call(target.id, target.value);
    }));

    document.getElementById(resetId)!.addEventListener('click', event => {
        const target = event.target as HTMLButtonElement;
        call(target.id);
    });

    document.getElementById(reloadId)!.addEventListener('click', event => {
        const target = event.target as HTMLButtonElement;
        call(target.id);
    });
});

function call(id : string, value : string | number | undefined = undefined) {
    const xhr = new XMLHttpRequest();
    if (value)
        xhr.open('GET', `/api/${id}?value=${value}`);
    else
        xhr.open('GET', `/api/${id}`);
    xhr.send();
}
