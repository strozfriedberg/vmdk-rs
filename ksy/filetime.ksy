meta:
  id: filetime 
  title: A file time is a 64-bit value that represents the number of 100-nanosecond intervals that have elapsed since 12:00 A.M. January 1, 1601 Coordinated Universal Time (UTC). 
  ks-version: 0.9
  tags:
    - ntfs windows
  endian: le
doc-ref: https://docs.microsoft.com/en-us/windows/win32/sysinfo/file-times
seq:
  - id: raw_value
    type: u8
instances:
  tickspersec:
    value: 10000000.as<u8>
  tickspermsec:
    value: 10000.as<u8>
  secsperday:
    value: 86400.as<u8>
  secsperhour:
    value: 3600
  secspermin:
    value: 60
  epochweekday:
    value: 1    # jan 1, 1601 was monday
  daysperweek:
    value: 7.as<u8>
  daysperquadricentennium:
    value: 365 * 400 + 97
  dayspernormalquadrennium:
    value: 365 * 4 + 1
    
  milliseconds: 
    value: ((raw_value % tickspersec) / tickspermsec).as<u8>
  time:
    value: (raw_value / tickspersec).as<u8>
  days0:
    value: (time / secsperday).as<u8>
  secondsinday:
    value: (time % secsperday).as<u4>
  hour:
    value: secondsinday / secsperhour
  minute:
    value: (secondsinday % secsperhour) / secspermin
  second:
    value: (secondsinday % secsperhour) % secspermin
  weekday:
    value: ((epochweekday + days0) % daysperweek).as<u1>
  cleaps:
    value: (( 3 * ((4 * days0 + 1227) / daysperquadricentennium) + 3 ) / 4).as<u1>
  days:
    value: (days0 + 28188 + cleaps).as<u8>
  years:
    value: ((20 * days - 2442) / (5 * dayspernormalquadrennium)).as<u4>
  yearday:
    value: (days - (years * dayspernormalquadrennium) / 4).as<u4>
  months:
    value: (64 * yearday) / 1959
  month:
    value: 'months < 14 ? months - 1 : months - 13'
  year:
    value: 'months < 14 ? years + 1524 : years + 1525'
  day:
    value: yearday - (1959 * months) / 64
  
  as_string:
    value: |
      year.to_s + '-' 
      + (month.to_s.length < 2 ? '0' + month.to_s : month.to_s) + '-' 
      + (day.to_s.length < 2 ? '0' + day.to_s : day.to_s) + 'T'
      + (hour.to_s.length < 2 ? '0' + hour.to_s : hour.to_s) + ':'
      + (minute.to_s.length < 2 ? '0' + minute.to_s : minute.to_s) + ':'
      + (second.to_s.length < 2 ? '0' + second.to_s : second.to_s)
