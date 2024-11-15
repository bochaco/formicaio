# Formicaio

Le formiche sono insetti sociali che vivono in colonie e sono 
note per la loro organizzazione e cooperazione.

Ants are social insects that live in colonies and are 
known for their organization and cooperation.

Simplify your decentralized experience with this intuitive application, 
designed to streamline your daily tasks when running nodes from home 
on peer-to-peer (P2P) networks. Seamlessly participate in online 
communities using the integrated Nostr client, and manage your 
digital assets with ease through the built-in wallet. Receive, send,
and store tokens, rewards, and coins earned from running nodes or received 
from third-party sources, all within a single, user-friendly interface.

<img src="img/screenshot_01.png" width="800" height="436" />
<img src="img/screenshot_02.png" width="600" height="633" />
<img src="img/screenshot_03.png" width="800" height="438" />

## How to use:

### UmbrelOS

This application has not yet been published on the official UmbrelOS app store. However, you can still install and run it on any UmbrelOS device using the [Formicaio community app store](https://github.com/bochaco/formicaio-app-store). To do this, simply add the GitHub URL (https://github.com/bochaco/formicaio-app-store) through the UmbrelOS user interface, as demonstrated in the following demo:

https://user-images.githubusercontent.com/10330103/197889452-e5cd7e96-3233-4a09-b475-94b754adc7a3.mp4

### Linux (amd64/arm64)

This application can also be launched on a Linux (amd64/arm64) machine using Docker Compose with the following commands:

```
$ git clone https://github.com/bochaco/formicaio
$ cd formicaio/deploy/local
$ docker compose up -d
```

Once Docker has completed pulling the images and starting the containers, the app will be running in the background, and you can access the Formicaio app from a web browser at `localhost:52100`

To see the logs you can simply use the following command:
```
$ docker compose logs -f
```

Upgrading the application (without stopping the running nodes instances) can be simply achieved by pulling the new Formicaio image and restarting the service:
```
$ docker compose pull formicaio
$ docker compose down formicaio
$ docker compose up formicaio -d
```

For stopping the Formicaio app and services simply run:
```
$ docker compose down
```

## Disclaimer

Please be aware that the Formicaio backend application, as well as the `safenode` binary running within each user-created node instance, utilizes third-party RPC services to retrieve information related to the Arbitrum L2 ledger.

Specifically, the Formicaio backend application queries the RPC server at `https://sepolia-rollup.arbitrum.io/rpc` to periodically check the current rewards balances for each node instance based on the configured (ETH) rewards addresses.

Risks:
- Privacy: Using third-party RPC services may expose your IP address and other metadata to those services. This means that the service provider can potentially track which addresses you are querying, which could lead to privacy concerns regarding your activity on the Arbitrum network.
- Data Exposure: Any data sent to the RPC service may be logged or monitored by the service provider, which could include sensitive information related to your node instances.

We recommend considering these risks when using the application and taking appropriate measures to protect your privacy.

## License

This project is licensed under the General Public License (GPL), version
3 ([LICENSE](http://www.gnu.org/licenses/gpl-3.0.en.html)).