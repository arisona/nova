import './style.css'

const contentsId = 'nova-contents';
const rangeIds = ['nova-red', 'nova-green', 'nova-blue', 'nova-brightness', 'nova-speed'];
const resetId = 'nova-reset';
const reloadId = 'nova-reload';
const selectedContent = 'nova-content';

syncServerState();
addListeners();

function syncServerState(): void {
    fetch('/api/get-state')
        .then(response => {
            if (!response.ok)
                throw new Error(response.status.toString());
            return response;
        })
        .then(response => response.json())
        .then(data => {
            rangeIds.forEach(id => {
                const range = document.getElementById(id) as HTMLInputElement;
                range.value = data[id];
            });
            const contents = document.getElementById(contentsId) as HTMLSelectElement;
            for (let i = 0; i < data[contentsId].length; ++i) {
                contents.appendChild(new Option(data[contentsId][i], i.toString(), false, i == data[selectedContent]));
            }
        })
        .catch(function (error) {
            console.error('Request failed ', error)
        });
}

function addListeners(): void {
    // add event listeners to ui elements
    document.getElementById(contentsId)!.addEventListener('change', event => {
        const target = event.target as HTMLSelectElement;
        fetch(`/api/${target.id}?value=${target.value}`);
    });

    rangeIds.forEach(id => document.getElementById(id)!.addEventListener('input', event => {
        const target = event.target as HTMLInputElement;
        fetch(`/api/${target.id}?value=${target.value}`);
    }));

    document.getElementById(resetId)!.addEventListener('click', event => {
        const target = event.target as HTMLButtonElement;
        fetch(`/api/${target.id}`);
    });

    document.getElementById(reloadId)!.addEventListener('click', event => {
        const target = event.target as HTMLButtonElement;
        fetch(`/api/${target.id}`);
    });
}
