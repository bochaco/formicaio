export async function get_addr_from_metamask() {
    if (typeof window.ethereum !== 'undefined') {
        try {
            // Request account access
            const accounts = await window.ethereum.request({ method: 'eth_requestAccounts' });
            return accounts[0]; // Return the first account
        } catch (error) {
            console.error('User denied account access:', error);
            return null;
        }
    } else {
        console.log('MetaMask is not installed. Please install it to use this feature.');
        return null;
    }
}