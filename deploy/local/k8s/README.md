# Running Formicaio with Podman and generate Kubernetes YAML file

## Install Podman
Formicaio can be deployed and run using [Podman](https://podman.io/) instead of Docker on Linux, Windows, or macOS. To get started, you'll need to install Podman by following the instructions available at https://podman.io/docs/installation.
You can choose to install either Podman Desktop or just the command-line interface (CLI), depending on your preference and the installation options available for your platform.
Be sure to follow the installation guide specific to your operating system, which will include executing the following two commands to initialize and start the Podman machine:
```
$ podman machine init
```
...and this second command (which may or may not be necessary):
```
$ podman machine start
```

## Run pod from the Kubernetes YAML file
Now you can simply use the Kubernetes `formicaio-pod.yaml` file found on this folder to launch the pod and Formicaio app:
```
$ podman play kube formicaio-pod.yaml
```

If you need to regenerate the `formicaio-pod.yaml` file, you can follow the steps below.

## Create a pod
First create an empty pod with network mode set to host so its container can share the network with the host:
```
$ podman pod create -p 52100:52100 --name formicaio-pod
```

## Add (and run) Formicaio container to the pod
Add the Formicaio container to the pod:
```
$ podman run --name formicaio -dt -v pod_volume_formicaio:/data -e DB_PATH=/data -e NODE_MGR_ROOT_DIR=/data --pod formicaio-pod docker.io/bochaco/formicaio:latest-native
```

## Generate Kubernetes YAML file
```
$ podman generate kube formicaio-pod > formicaio-pod.yaml
```

## Remove pod and files
If you want to remove to leave no trace of the files stored by the pod and/or Formicaio app, you can remove the pod and volume with the following commands:
```
$ podman pod rm formicaio-pod -f
$ podman volume rm pod_volume_formicaio
```