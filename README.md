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

Upgrading the application can be simply achieved by pulling the new image/s and restarting the services:
```
$ docker compose pull
$ docker compose down
$ docker compose up -d
```

For stopping the Formicaio app and services simply run:
```
$ docker compose down
```

## License

This project is licensed under the General Public License (GPL), version
3 ([LICENSE](http://www.gnu.org/licenses/gpl-3.0.en.html)).