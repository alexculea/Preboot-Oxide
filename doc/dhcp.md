## Quick overview of the protocol
Using UDP, a DHCP client broadcasts to 255.255.255.255 (port 67) a DHCP `Discover` message, including various information as indicated in the `tcpdump` below. The server responds on port 68 with an `Offer`, then the client confirms the interpreted offer with a `Request` containg correct server provided information, with the server responding with an `Ack` (Acknowledge) message from the server that all is OK.


## Example DHCP Messages captured using `tcpdump` where the boot happens over the network

```
tcpdump: listening on enp0s8, link-type EN10MB (Ethernet), snapshot length 262144 bytes
20:57:15.774458 08:00:27:e7:de:fe > ff:ff:ff:ff:ff:ff, ethertype IPv4 (0x0800), length 389: (tos 0x0, ttl 64, id 60577, offset 0, flags [none], proto UDP (17), length 375)
    0.0.0.0.68 > 255.255.255.255.67: [udp sum ok] BOOTP/DHCP, Request from 08:00:27:e7:de:fe, length 347, xid 0x72dd3b0e, Flags [Broadcast] (0x8000)
          Client-Ethernet-Address 08:00:27:e7:de:fe
          Vendor-rfc1048 Extensions
            Magic Cookie 0x63825363
            DHCP-Message (53), length 1: Discover
            MSZ (57), length 2: 1472
            Parameter-Request (55), length 35: 
              Subnet-Mask (1), Time-Zone (2), Default-Gateway (3), Time-Server (4)
              IEN-Name-Server (5), Domain-Name-Server (6), Hostname (12), BS (13)
              Domain-Name (15), RP (17), EP (18), RSZ (22)
              TTL (23), BR (28), YD (40), YS (41)
              NTP (42), Vendor-Option (43), Requested-IP (50), Lease-Time (51)
              Server-ID (54), RN (58), RB (59), Vendor-Class (60)
              TFTP (66), BF (67), GUID (97), Unknown (128)
              Unknown (129), Unknown (130), Unknown (131), Unknown (132)
              Unknown (133), Unknown (134), Unknown (135)
            GUID (97), length 17: 0.41.217.207.159.93.37.3.72.177.75.204.207.121.31.106.161
            NDI (94), length 3: 1.3.0
            ARCH (93), length 2: 7
            Vendor-Class (60), length 32: "PXEClient:Arch:00007:UNDI:003000"
20:57:15.774607 08:00:27:5e:93:d6 > ff:ff:ff:ff:ff:ff, ethertype IPv4 (0x0800), length 342: (tos 0x10, ttl 128, id 0, offset 0, flags [none], proto UDP (17), length 328)
    10.5.5.1.67 > 255.255.255.255.68: [udp sum ok] BOOTP/DHCP, Reply, length 300, xid 0x72dd3b0e, Flags [Broadcast] (0x8000)
          Your-IP 10.5.5.13
          Server-IP 10.5.5.1
          Client-Ethernet-Address 08:00:27:e7:de:fe
          file "debian-installer/amd64/bootnetx64.efi"
          Vendor-rfc1048 Extensions
            Magic Cookie 0x63825363
            DHCP-Message (53), length 1: Offer
            Server-ID (54), length 4: 10.5.5.1
            Lease-Time (51), length 4: 472
            Subnet-Mask (1), length 4: 255.255.255.0
20:57:19.731131 08:00:27:e7:de:fe > ff:ff:ff:ff:ff:ff, ethertype IPv4 (0x0800), length 401: (tos 0x0, ttl 64, id 60578, offset 0, flags [none], proto UDP (17), length 387)
    0.0.0.0.68 > 255.255.255.255.67: [udp sum ok] BOOTP/DHCP, Request from 08:00:27:e7:de:fe, length 359, xid 0x72dd3b0e, Flags [Broadcast] (0x8000)
          Client-Ethernet-Address 08:00:27:e7:de:fe
          Vendor-rfc1048 Extensions
            Magic Cookie 0x63825363
            DHCP-Message (53), length 1: Request
            Server-ID (54), length 4: 10.5.5.1
            Requested-IP (50), length 4: 10.5.5.13
            MSZ (57), length 2: 65280
            Parameter-Request (55), length 35: 
              Subnet-Mask (1), Time-Zone (2), Default-Gateway (3), Time-Server (4)
              IEN-Name-Server (5), Domain-Name-Server (6), Hostname (12), BS (13)
              Domain-Name (15), RP (17), EP (18), RSZ (22)
              TTL (23), BR (28), YD (40), YS (41)
              NTP (42), Vendor-Option (43), Requested-IP (50), Lease-Time (51)
              Server-ID (54), RN (58), RB (59), Vendor-Class (60)
              TFTP (66), BF (67), GUID (97), Unknown (128)
              Unknown (129), Unknown (130), Unknown (131), Unknown (132)
              Unknown (133), Unknown (134), Unknown (135)
            GUID (97), length 17: 0.41.217.207.159.93.37.3.72.177.75.204.207.121.31.106.161
            NDI (94), length 3: 1.3.0
            ARCH (93), length 2: 7
            Vendor-Class (60), length 32: "PXEClient:Arch:00007:UNDI:003000"
20:57:19.731267 08:00:27:5e:93:d6 > ff:ff:ff:ff:ff:ff, ethertype IPv4 (0x0800), length 342: (tos 0x10, ttl 128, id 0, offset 0, flags [none], proto UDP (17), length 328)
    10.5.5.1.67 > 255.255.255.255.68: [udp sum ok] BOOTP/DHCP, Reply, length 300, xid 0x72dd3b0e, Flags [Broadcast] (0x8000)
          Your-IP 10.5.5.13
          Server-IP 10.5.5.1
          Client-Ethernet-Address 08:00:27:e7:de:fe
          file "debian-installer/amd64/bootnetx64.efi"
          Vendor-rfc1048 Extensions
            Magic Cookie 0x63825363
            DHCP-Message (53), length 1: ACK
            Server-ID (54), length 4: 10.5.5.1
            Lease-Time (51), length 4: 468
            Subnet-Mask (1), length 4: 255.255.255.0
20:57:19.731902 08:00:27:e7:de:fe > 08:00:27:5e:93:d6, ethertype IPv4 (0x0800), length 122: (tos 0x0, ttl 64, id 60579, offset 0, flags [none], proto UDP (17), length 108)
    10.5.5.13.1284 > 10.5.5.1.69: [udp sum ok] TFTP, length 80, RRQ "debian-installer/amd64/bootnetx64.efi" octet tsize 0 blksize 1468 windowsize 4
```

The output above was produced using this command:
```BASH
sudo tcpdump -i enp0s3 port 67 or port 68 -e -n -vv
```

Wireshark can open `tcpdump` capture files. To start capturing:
```BASH
sudo tcpdump -i enp0s8 -s 65535 -w ./network.dmp
```