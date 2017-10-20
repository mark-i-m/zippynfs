
import numpy as np
import matplotlib.pyplot as plt

#[aws_aws, aws_lh, lh_lh]
no_fail = [1.4, 145.9, 0.3]
fail = [5.6, 294.7, 3.5]

ind = np.arange(3)  # the x locations for the groups
width = 0.35       # the width of the bars

colors=['#DEA4BD', '#B74576','#A6CEE3', '#1F78B4', '#969696', '#252525']

rects1 = plt.bar(ind, fail, width, color=colors[0])
rects2 = plt.bar(ind + width, no_fail, width, color=colors[1])

plt.yscale('log')

plt.yticks([0.1, 1.0, 10, 100, 1000], ['100ms', '1s', '10s', '100s', '1ks'])

plt.ylabel('Latency')

plt.xticks([i + width  for i in [0,1,2]], ('Client: AWS, Server: AWS', 'Client: seclab8, Server: AWS', 'Client: seclab8, Server: seclab8'))
plt.xlabel('Location of Client and Server')

plt.xlim(0, 2+2*width)

plt.title('Latency of 10MiB UNSTABLE Writes with Failure Just Before COMMIT')

plt.legend((rects1[0], rects2[0]), ['Failure', 'No failure'])

plt.grid(True)

plt.show()

