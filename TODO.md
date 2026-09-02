
`autobahn status` should feature the folder prominently followed by the project id and then each connection
status


<b>~/.fny</b> fny
  dev@lager:~/.fny
    status:  inactive (unreachable at last start) (synchonized  1 cycle (10s ago)) error etc
    conflicts: ...
    mode: two-way-resolved
  claude@reinhardt.de:~/.fny
    status:  inactive (unreachable at last start)
  ubuntu@halle.steinbach.de:~/.fny
    status:  inactive (unreachable at last start)


---


`autobahn up` should return connection statuses alone.
check is for connected
x is for disconnected
grayed with no symbol means the host is disabled


<b>aws</b>
  ✓  lager
  ✓  reinhardt.de
  ✓  halle.steinbach.de
  ✗  pruefstand.steinbach.de
  <span class="disabled">  soros</span>
<b>fny</b>
  ✓  lager
  ✓  reinhardt.de
  ✓  halle.steinbach.de
  ✗  pruefstand.steinbach.de

---
