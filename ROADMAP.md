# Roadmap (random order)

- Remove the need for IP configuration, refer from the IP of the source network interface instead ✅
- Add support for serving TFTP ✅
- Add service configuration template file for systemd ✅
- Ensure DHCP service can run alongside dhclient and isc-dhcp-server ✅
- Mitigate DOS vulnerabilities by ensuring sessions are limited in number and periodically removing old session entries ✅
- Add support for YAML configuration ✅
- Allow configuring listen interfaces for the TFTP service ✅
- Good coverage of automated integration testing
- Publish APT package repositories
- Provide RPM packages at every release
- Match 90% of the `tftp-hpa` TFTP file server performance
- Monitor network interfaces and rebind and refresh inferred IP information provided to clients 

