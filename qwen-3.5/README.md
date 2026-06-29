## Running Qwen3.5-397B on Hotaisle with FP8 quantization

It fits with 256K context on a 4 MI300X GPUs for $8/hr (minimum 2 hours).
The initial startup for the model is about 15-20 minites because of the massive download of the model.

To test this procedure out you can run the 1xGPU VM which is just $2/hr and billed by the minute. Everything should work except actually running the server, it just doesn't fit. You can change the `run.sh` script to `run06.sh` and it will run on 1xGPU VM, but the model is quite unfit for coding tasks.
Once you make sure everything works, kill the small VM, start a bigger one and rerun the deploy. Don't forget to change the IP address and server key in the Wireguard config, and restart it.

### 0. Register at HotAisle

`ssh admin.hotaisle.app`

Spin up a VM with 4 GPUs and note its IP address.

`export HOTAISLE_IP=<server ip>`

### 1. Install Wireguard if you want to use it from local machine

```
brew install wireguard-tools

mkdir -p /usr/local/etc/wireguard
# Generate keys
wg genkey | tee /usr/local/etc/wireguard/client_private.key | wg pubkey > /usr/local/etc/wireguard/client_public.key
chmod 600  /usr/local/etc/wireguard/client_private.key

# Create config
cat > /usr/local/etc/wireguard/wg0.conf << 'EOF'
[Interface]
PrivateKey = <contents of client_private.key>
Address = 10.0.0.2/24

[Peer]
PublicKey = <server public key>
Endpoint = $HOTAISLE_IP:51820
AllowedIPs = 10.0.0.1/32
PersistentKeepalive = 25
EOF
```

Server public key you will learn at the end of the Ansible playbook run and the IP address is what HotAisle gave you.

### 2. Deploy Ansible playbook

Copy the `vllm-*.whl` file from GitHub releases page to the `vllm-dist`.

`brew install ansible`

`ANSIBLE_HOST_KEY_CHECKING=False ansible-playbook -i "HOTAISLE_IP," -u hotaisle setup.yml`

The comma at the end of the IP address is important.
It will run for about 5-10 minites and at the end will tell you the server public key. You need to go back to the Wireguard client config and enter it there, then start/restart Wireguard: `wg-quick up wg0`

### 3. Run the server

Login to the server with `ssh hotaisle@$HOTAISLE_IP` and run the server:
`./run.sh`. It will take a while to download the model and start the server.
Once you see `INFO:     Application startup complete.` you can try to connect to it from the client: `curl -v http://10.0.0.1:8000/v1/models`.

### 4. Setup the opencode

Copy the `opencode.json` file to `~/.config/opencode/` and start the opencode.

### 5. Wtf is tcp-stack?

This is the eval so to speak. This is what `opencode` created fully autonomously after given the following prompt and answering its questions:

```
We want to create a from scratch implementation of the TCP protocol in Rust, running in the user space and using Linux raw IP sockets
```

Judge for yourself.
